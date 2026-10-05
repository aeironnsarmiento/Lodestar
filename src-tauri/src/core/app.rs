//! The backend service that Tauri commands call into. It owns the store, providers,
//! Java runtimes, the process supervisor and the event sink.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
use crate::supervisor::{ServerState, StateChange, Supervisor, DEFAULT_STOP_TIMEOUT};

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
        let java = JavaManager::new(config.paths.java_runtimes_dir(), downloader, config.adoptium_api.clone());
        let supervisor = Supervisor::with_stop_timeout(events.clone(), config.stop_timeout);
        for inst in store.list() {
            supervisor.load_history(&inst.id, &log_path(&config.paths.server_dir(&inst.id)));
        }
        let app = Arc::new(Self { store, events, providers, java, supervisor, java_override: config.java_override });
        app.install_hooks();
        Ok(app)
    }

    fn install_hooks(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        self.supervisor.add_hook(Arc::new(move |change| {
            if let Some(app) = weak.upgrade() {
                app.on_state_change(change);
            }
        }));
    }

    /// Reacts to lifecycle transitions (runs on the supervisor's task; must not block).
    fn on_state_change(&self, change: &StateChange) {
        if change.state == ServerState::Online {
            if let Ok(inst) = self.store.get(&change.id) {
                let name = inst.op_name.trim();
                if !name.is_empty() {
                    let _ = self.supervisor.send_command(&change.id, &format!("op {name}"));
                }
            }
        }
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
        let inst = self.store.update(inst)?;
        self.notify_instances_changed();
        Ok(inst)
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
