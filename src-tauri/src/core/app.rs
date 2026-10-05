//! The backend service that Tauri commands call into. It owns the store, providers,
//! Java runtimes, the process supervisor and the event sink.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use super::events::{self, Events};
use super::instance::{Instance, NewInstance, ServerType};
use super::paths::Paths;
use super::settings::AppSettings;
use super::store::Store;
use crate::download::Downloader;
use crate::java::{InstalledRuntime, JavaManager};
use crate::providers::{Endpoints, Providers, VersionEntry};
use crate::supervisor::console::log_path;
use crate::lifecycle::crash::{CrashPolicy, BACKOFF};
use crate::lifecycle::keep_awake::KeepAwake;
use crate::lifecycle::scheduler::ScheduleState;
use crate::lifecycle::{Clock, SystemClock};
use crate::playit::{PlayitConfig, PlayitManager};
use crate::supervisor::{ServerState, Supervisor, DEFAULT_STOP_TIMEOUT};
use crate::worlds::properties::remove_from_player_file;

/// Names in `a` that are not in `b`, ignoring case.
fn missing_from(a: &[String], b: &[String]) -> Vec<String> {
    a.iter().filter(|n| !b.iter().any(|m| m.eq_ignore_ascii_case(n))).cloned().collect()
}

/// Where the app's data lives and which services it talks to.
#[derive(Clone)]
pub struct AppConfig {
    pub paths: Paths,
    pub endpoints: Endpoints,
    pub adoptium_api: String,
    pub download_backoff: Duration,
    pub stop_timeout: Duration,
    /// Runs servers with this program instead of the managed Java. Tests use it to
    /// substitute `fake_mc` (KTD5); the app never sets it.
    pub java_override: Option<std::path::PathBuf>,
    /// Time source for crash windows and restart schedules.
    pub clock: Arc<dyn Clock>,
    /// Delays before the automatic restarts after a crash (KTD13).
    pub crash_backoff: [Duration; 3],
    pub playit: PlayitConfig,
}

impl AppConfig {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            endpoints: Endpoints::default(),
            adoptium_api: JavaManager::DEFAULT_API.into(),
            download_backoff: Duration::from_secs(1),
            stop_timeout: DEFAULT_STOP_TIMEOUT,
            java_override: None,
            clock: Arc::new(SystemClock),
            crash_backoff: BACKOFF,
            playit: PlayitConfig::default(),
        }
    }
}

pub struct App {
    pub store: Arc<Store>,
    pub events: Events,
    pub providers: Providers,
    pub java: JavaManager,
    pub supervisor: Arc<Supervisor>,
    pub java_override: Option<std::path::PathBuf>,
    pub clock: Arc<dyn Clock>,
    pub crash_backoff: [Duration; 3],
    pub crash: Mutex<CrashPolicy>,
    pub schedules: Mutex<HashMap<String, ScheduleState>>,
    pub keep_awake: KeepAwake,
    pub playit: Arc<PlayitManager>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub task: String,
    pub label: String,
    pub done: u64,
    pub total: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntimeInfo {
    #[serde(flatten)]
    pub runtime: InstalledRuntime,
    pub in_use: bool,
}

impl App {
    pub fn new(config: AppConfig, events: Events) -> Result<Arc<Self>> {
        let store = Arc::new(Store::open(config.paths.clone())?);
        let downloader = Downloader::new().with_backoff(config.download_backoff);
        let providers = Providers::new(config.paths.clone(), downloader.clone(), config.endpoints.clone());
        let java = JavaManager::new(config.paths.java_runtimes_dir(), downloader.clone(), config.adoptium_api.clone());
        let supervisor = Supervisor::with_stop_timeout(events.clone(), config.stop_timeout);
        for inst in store.list() {
            supervisor.load_history(&inst.id, &log_path(&config.paths.server_dir(&inst.id)));
        }
        let playit = PlayitManager::new(config.paths.playit_dir(), downloader, config.playit.clone(), events.clone(), supervisor.clone());
        let app = Arc::new(Self {
            store,
            events,
            providers,
            java,
            supervisor,
            java_override: config.java_override,
            clock: config.clock,
            crash_backoff: config.crash_backoff,
            crash: Mutex::new(CrashPolicy::default()),
            schedules: Mutex::new(HashMap::new()),
            keep_awake: KeepAwake::new(),
            playit,
        });
        app.install_hooks();
        Ok(app)
    }

    pub fn paths(&self) -> &Paths {
        self.store.paths()
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.supervisor.is_running(id)
    }

    pub fn notify_instances_changed(&self) {
        events::emit(&*self.events, events::INSTANCES_CHANGED, &());
    }

    /// A download progress callback that emits `task-progress` at most once per
    /// whole percent (or per MiB when the size is unknown).
    pub fn progress(&self, task: &str, label: &str) -> impl Fn(u64, Option<u64>) + Send + Sync + 'static {
        let events = self.events.clone();
        let task = task.to_string();
        let label = label.to_string();
        let last = AtomicU64::new(u64::MAX);
        move |done, total| {
            let bucket = match total {
                Some(t) if t > 0 => done * 100 / t,
                _ => done >> 20,
            };
            if last.swap(bucket, Ordering::Relaxed) != bucket {
                events::emit(
                    &*events,
                    events::TASK_PROGRESS,
                    &TaskProgress { task: task.clone(), label: label.clone(), done, total },
                );
            }
        }
    }

    pub fn list_instances(&self) -> Vec<Instance> {
        self.store.list()
    }

    pub fn create_instance(&self, new: NewInstance) -> Result<Instance> {
        let inst = self.store.create(new)?;
        self.notify_instances_changed();
        Ok(inst)
    }

    pub fn update_instance(&self, inst: Instance) -> Result<Instance> {
        let before = self.store.get(&inst.id)?;
        let inst = self.store.update(inst)?;
        self.sync_player_lists(&before, &inst);
        self.notify_instances_changed();
        Ok(inst)
    }

    /// Applies whitelist and operator edits. Online, they go through the console right
    /// away; otherwise removals are made in the server's list files and additions wait
    /// for the next start (see `on_state_change`).
    fn sync_player_lists(&self, before: &Instance, after: &Instance) {
        let online = self.supervisor.state(&after.id) == ServerState::Online;
        let server_dir = self.paths().server_dir(&after.id);
        let lists = [
            (&before.whitelist, &after.whitelist, "whitelist add", "whitelist remove", "whitelist.json"),
            (&before.operators, &after.operators, "op", "deop", "ops.json"),
        ];
        for (old, new, add, remove, file) in lists {
            // The host stays an operator even when dropped from the extra list.
            let removed: Vec<String> = missing_from(old, new)
                .into_iter()
                .filter(|n| file != "ops.json" || !n.eq_ignore_ascii_case(&after.op_name))
                .collect();
            if online {
                for name in &removed {
                    let _ = self.supervisor.send_command(&after.id, &format!("{remove} {name}"));
                }
                for name in missing_from(new, old) {
                    let _ = self.supervisor.send_command(&after.id, &format!("{add} {name}"));
                }
            } else if let Err(e) = remove_from_player_file(&server_dir.join(file), &removed) {
                eprintln!("lodestar: could not update {file}: {e:#}");
            }
        }
        if online && before.properties.white_list != after.properties.white_list {
            let toggle = if after.properties.white_list { "whitelist on" } else { "whitelist off" };
            let _ = self.supervisor.send_command(&after.id, toggle);
        }
    }

    pub fn delete_instance(&self, id: &str) -> Result<()> {
        self.store.delete(id, self.is_running(id))?;
        self.notify_instances_changed();
        Ok(())
    }

    pub fn settings(&self) -> AppSettings {
        self.store.settings()
    }

    pub fn set_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        self.store.set_settings(settings)
    }

    pub async fn versions(&self, server_type: ServerType) -> Result<Vec<VersionEntry>> {
        self.providers.versions(server_type).await
    }

    /// Java majors used by running servers.
    fn java_in_use(&self) -> Vec<u32> {
        self.store
            .list()
            .iter()
            .filter(|i| self.is_running(&i.id))
            .filter_map(|i| i.java_major.map(crate::java::target_major))
            .collect()
    }

    pub fn java_runtimes(&self) -> Vec<JavaRuntimeInfo> {
        let in_use = self.java_in_use();
        self.java
            .installed()
            .into_iter()
            .map(|runtime| JavaRuntimeInfo { in_use: in_use.contains(&runtime.major), runtime })
            .collect()
    }

    pub fn remove_java_runtime(&self, major: u32) -> Result<()> {
        self.java.remove(major, self.java_in_use().contains(&major))
    }
}
