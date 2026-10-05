//! Runs launch specs under supervision: lifecycle state machine, live and persisted
//! console, players, stop/kill, port checks and metrics (U5).

pub mod console;
pub mod job_object;
pub mod metrics;
pub mod ports;
pub mod process;
pub mod state;

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch, Notify};

use crate::core::events::{self, Events};
use crate::providers::LaunchSpec;
use console::{Console, ConsoleLine};
use job_object::JobObject;
use metrics::{ProcessMetrics, Sampler};
pub use state::{ServerState, StopReason};
use state::{is_ready_line, parse_player_event, PlayerEvent};

/// How long a graceful `stop` may take before the process is killed.
pub const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(15);
const BATCH_INTERVAL: Duration = Duration::from_millis(50);
const METRICS_INTERVAL: Duration = Duration::from_secs(2);

/// A lifecycle transition, delivered to hooks (crash policy, keep-awake, worlds...).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StateChange {
    pub id: String,
    pub state: ServerState,
    pub previous: ServerState,
    /// Set on the transition out of a running process: why it was stopped, if it was.
    pub reason: Option<StopReason>,
    pub exit_code: Option<i32>,
}

pub type Hook = Arc<dyn Fn(&StateChange) + Send + Sync>;

/// Everything the UI shows about a running (or stopped) instance.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub state: ServerState,
    pub port: u16,
    pub pid: Option<u32>,
    pub players: Vec<String>,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub uptime_secs: Option<u64>,
    /// Last problem worth showing (launch failure, crash notice).
    pub message: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ConsoleBatch {
    id: String,
    lines: Vec<ConsoleLine>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PlayersChanged {
    id: String,
    players: Vec<String>,
}

struct Inner {
    name: String,
    state: ServerState,
    port: u16,
    pid: Option<u32>,
    stdin: Option<mpsc::UnboundedSender<String>>,
    requested: Option<StopReason>,
    players: BTreeSet<String>,
    console: Console,
    metrics: ProcessMetrics,
    started_at: Option<Instant>,
    message: Option<String>,
    /// Bumped on every start so stale timers from an earlier run do nothing.
    session: u64,
}

struct Runtime {
    id: String,
    inner: Mutex<Inner>,
    state_tx: watch::Sender<ServerState>,
    kill: Notify,
}

impl Runtime {
    fn snapshot(&self) -> Snapshot {
        let i = self.inner.lock().unwrap();
        Snapshot {
            id: self.id.clone(),
            state: i.state,
            port: i.port,
            pid: i.pid,
            players: i.players.iter().cloned().collect(),
            cpu_percent: i.metrics.cpu_percent,
            memory_bytes: i.metrics.memory_bytes,
            uptime_secs: i.started_at.map(|t| t.elapsed().as_secs()),
            message: i.message.clone(),
        }
    }
}

pub struct Supervisor {
    events: Events,
    job: Option<JobObject>,
    runtimes: Mutex<HashMap<String, Arc<Runtime>>>,
    hooks: Mutex<Vec<Hook>>,
    stop_timeout: Duration,
    metrics_started: AtomicBool,
}

impl Supervisor {
    pub fn new(events: Events) -> Arc<Self> {
        Self::with_stop_timeout(events, DEFAULT_STOP_TIMEOUT)
    }

    pub fn with_stop_timeout(events: Events, stop_timeout: Duration) -> Arc<Self> {
        let job = match JobObject::new_kill_on_close() {
            Ok(j) => Some(j),
            Err(e) => {
                eprintln!("glasscraft: no job object, child processes may outlive the app: {e:#}");
                None
            }
        };
        Arc::new(Self {
            events,
            job,
            runtimes: Mutex::new(HashMap::new()),
            hooks: Mutex::new(Vec::new()),
            stop_timeout,
            metrics_started: AtomicBool::new(false),
        })
    }

    pub fn job(&self) -> Option<&JobObject> {
        self.job.as_ref()
    }

    pub fn add_hook(&self, hook: Hook) {
        self.hooks.lock().unwrap().push(hook);
    }

    fn runtime(&self, id: &str) -> Arc<Runtime> {
        self.runtimes
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_insert_with(|| {
                Arc::new(Runtime {
                    id: id.to_string(),
                    inner: Mutex::new(Inner {
                        name: id.to_string(),
                        state: ServerState::Stopped,
                        port: 0,
                        pid: None,
                        stdin: None,
                        requested: None,
                        players: BTreeSet::new(),
                        console: Console::new(),
                        metrics: ProcessMetrics::default(),
                        started_at: None,
                        message: None,
                        session: 0,
                    }),
                    state_tx: watch::channel(ServerState::Stopped).0,
                    kill: Notify::new(),
                })
            })
            .clone()
    }

    /// Makes the previous app session's console log readable (read-only) before the
    /// server is started again.
    pub fn load_history(&self, id: &str, log_path: &Path) {
        let rt = self.runtime(id);
        let mut i = rt.inner.lock().unwrap();
        if i.console.is_empty() && !i.state.has_process() {
            i.console.load_history(log_path);
        }
    }

    /// Moves the instance to `state`, notifying the UI and hooks. Called without locks.
    fn transition(&self, rt: &Runtime, state: ServerState, reason: Option<StopReason>, exit_code: Option<i32>) {
        let previous = {
            let mut i = rt.inner.lock().unwrap();
            let prev = i.state;
            i.state = state;
            if !state.has_process() {
                i.started_at = None;
                i.pid = None;
                i.metrics = ProcessMetrics::default();
            }
            prev
        };
        rt.state_tx.send_replace(state);
        events::emit(&*self.events, events::INSTANCE_STATE, &rt.snapshot());
        let change = StateChange { id: rt.id.clone(), state, previous, reason, exit_code };
        let hooks: Vec<Hook> = self.hooks.lock().unwrap().clone();
        for hook in hooks {
            hook(&change);
        }
    }

    fn set_message(&self, rt: &Runtime, message: Option<String>) {
        rt.inner.lock().unwrap().message = message;
    }

    /// Claims the instance's port and enters Preparing. Refused when the instance is
    /// already running or the port is taken (naming the instance that holds it, R4).
    pub fn prepare(&self, id: &str, name: &str, port: u16) -> Result<()> {
        let rt = {
            let map = self.runtimes.lock().unwrap();
            for other in map.values().filter(|r| r.id != id) {
                let o = other.inner.lock().unwrap();
                if o.state.is_active() && o.port == port {
                    bail!(
                        "Port {port} is already used by \"{}\", which is running. Give this server a different port or stop \"{}\" first.",
                        o.name,
                        o.name
                    );
                }
            }
            drop(map);
            self.runtime(id)
        };
        {
            let mut i = rt.inner.lock().unwrap();
            if i.state.is_active() {
                bail!("\"{}\" is already running.", i.name);
            }
            if !ports::port_available(port) {
                bail!("Port {port} is in use by another program. Close it or give this server a different port.");
            }
            i.name = name.to_string();
            i.port = port;
            i.message = None;
        }
        self.transition(&rt, ServerState::Preparing, None, None);
        Ok(())
    }

    /// Leaves Crashed for Stopped without restarting, keeping a notice (the crash
    /// policy gave up).
    pub fn give_up(&self, id: &str, message: String) {
        let rt = self.runtime(id);
        if rt.inner.lock().unwrap().state != ServerState::Crashed {
            return;
        }
        self.set_message(&rt, Some(message));
        self.transition(&rt, ServerState::Stopped, None, None);
    }

    /// Leaves Preparing without starting (provisioning failed or was cancelled).
    pub fn abort_prepare(&self, id: &str, message: Option<String>) {
        let rt = self.runtime(id);
        if rt.inner.lock().unwrap().state != ServerState::Preparing {
            return;
        }
        self.set_message(&rt, message);
        self.transition(&rt, ServerState::Stopped, None, None);
    }

    /// Spawns the spec. The instance must be Preparing (see [`Supervisor::prepare`]).
    pub fn start(self: &Arc<Self>, id: &str, spec: &LaunchSpec, log_path: &Path) -> Result<()> {
        let rt = self.runtime(id);
        let session = {
            let mut i = rt.inner.lock().unwrap();
            if i.state != ServerState::Preparing {
                bail!("\"{}\" is not ready to start ({:?}).", i.name, i.state);
            }
            if let Err(e) = i.console.open_log(log_path) {
                eprintln!("glasscraft: console log unavailable: {e}");
            }
            i.session += 1;
            i.session
        };

        let spawned = match process::spawn(spec, self.job.as_ref()) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("{e:#}");
                {
                    let mut i = rt.inner.lock().unwrap();
                    i.console.push(format!("[Glasscraft] Launch failed: {msg}"));
                    i.console.close_log();
                }
                self.flush(&rt);
                self.set_message(&rt, Some(format!("Launch failed: {msg}")));
                self.transition(&rt, ServerState::Stopped, None, None);
                return Err(anyhow!("Launch failed: {msg}"));
            }
        };
        let process::Spawned { mut child, mut stdin, stdout, stderr } = spawned;

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        {
            let mut i = rt.inner.lock().unwrap();
            i.pid = child.id();
            i.stdin = Some(tx);
            i.requested = None;
            i.players.clear();
            i.started_at = Some(Instant::now());
        }
        self.transition(&rt, ServerState::Starting, None, None);
        self.ensure_metrics_loop();

        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if stdin.write_all(format!("{line}\n").as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        let out_task = {
            let (sup, rt) = (self.clone(), rt.clone());
            tokio::spawn(async move { process::read_lines(stdout, |l| sup.on_line(&rt, l)).await })
        };
        let err_task = {
            let (sup, rt) = (self.clone(), rt.clone());
            tokio::spawn(async move { process::read_lines(stderr, |l| sup.on_line(&rt, l)).await })
        };

        let alive = Arc::new(AtomicBool::new(true));
        {
            let (sup, rt, alive) = (self.clone(), rt.clone(), alive.clone());
            tokio::spawn(async move {
                while alive.load(Ordering::Relaxed) {
                    tokio::time::sleep(BATCH_INTERVAL).await;
                    sup.flush(&rt);
                }
            });
        }

        let (sup, rt2) = (self.clone(), rt.clone());
        tokio::spawn(async move {
            let status = tokio::select! {
                s = child.wait() => s,
                _ = rt2.kill.notified() => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };
            // Let the readers drain the last output before reporting the exit.
            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                let _ = out_task.await;
                let _ = err_task.await;
            })
            .await;
            alive.store(false, Ordering::Relaxed);
            let code = status.ok().and_then(|s| s.code());
            sup.on_exit(&rt2, session, code);
        });
        Ok(())
    }

    fn on_line(&self, rt: &Runtime, line: String) {
        let mut became_online = false;
        let mut players_changed = None;
        {
            let mut i = rt.inner.lock().unwrap();
            if i.state == ServerState::Starting && is_ready_line(&line) {
                became_online = true;
            }
            match parse_player_event(&line) {
                Some(PlayerEvent::Joined(n)) => {
                    i.players.insert(n);
                    players_changed = Some(i.players.iter().cloned().collect::<Vec<_>>());
                }
                Some(PlayerEvent::Left(n)) => {
                    i.players.remove(&n);
                    players_changed = Some(i.players.iter().cloned().collect::<Vec<_>>());
                }
                None => {}
            }
            i.console.push(line);
        }
        if let Some(players) = players_changed {
            events::emit(&*self.events, events::PLAYERS, &PlayersChanged { id: rt.id.clone(), players });
        }
        if became_online {
            self.flush(rt);
            self.transition(rt, ServerState::Online, None, None);
        }
    }

    fn flush(&self, rt: &Runtime) {
        let lines = rt.inner.lock().unwrap().console.take_pending();
        if !lines.is_empty() {
            events::emit(&*self.events, events::CONSOLE_BATCH, &ConsoleBatch { id: rt.id.clone(), lines });
        }
    }

    fn on_exit(&self, rt: &Runtime, session: u64, code: Option<i32>) {
        let (reason, next) = {
            let mut i = rt.inner.lock().unwrap();
            if i.session != session {
                return;
            }
            let reason = i.requested.take();
            i.stdin = None;
            let had_players = !i.players.is_empty();
            i.players.clear();
            let code_text = code.map(|c| c.to_string()).unwrap_or_else(|| "?".into());
            let next = if reason.is_some() {
                i.console.push(format!("[Glasscraft] Server stopped (exit code {code_text})."));
                ServerState::Stopped
            } else {
                i.console.push(format!("[Glasscraft] Server exited unexpectedly (exit code {code_text})."));
                i.message = Some(format!("The server exited unexpectedly (exit code {code_text})."));
                ServerState::Crashed
            };
            i.console.close_log();
            if had_players {
                drop(i);
                events::emit(&*self.events, events::PLAYERS, &PlayersChanged { id: rt.id.clone(), players: vec![] });
            }
            (reason, next)
        };
        self.flush(rt);
        self.transition(rt, next, reason, code);
    }

    /// Asks the server to stop (`stop` over stdin) and kills it if it is still running
    /// after the stop timeout. Stopping a crashed instance acknowledges the crash.
    pub fn stop(self: &Arc<Self>, id: &str, reason: StopReason) -> Result<()> {
        let rt = self.runtime(id);
        let session = {
            let mut i = rt.inner.lock().unwrap();
            match i.state {
                ServerState::Starting | ServerState::Online => {
                    i.requested = Some(reason);
                    if let Some(tx) = &i.stdin {
                        let _ = tx.send("stop".into());
                    }
                    i.session
                }
                ServerState::Stopping => {
                    // Keep the first reason, but a reset/restart must not look like a user stop.
                    if i.requested.is_none() {
                        i.requested = Some(reason);
                    }
                    return Ok(());
                }
                ServerState::Crashed => {
                    drop(i);
                    self.transition(&rt, ServerState::Stopped, Some(reason), None);
                    return Ok(());
                }
                ServerState::Preparing => bail!("The server is still being prepared."),
                ServerState::Stopped => return Ok(()),
            }
        };
        self.transition(&rt, ServerState::Stopping, Some(reason), None);
        let (rt2, timeout) = (rt.clone(), self.stop_timeout);
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            let still = {
                let i = rt2.inner.lock().unwrap();
                i.session == session && i.state == ServerState::Stopping
            };
            if still {
                rt2.inner.lock().unwrap().console.push("[Glasscraft] The server did not stop in time; forcing it to close.".into());
                rt2.kill.notify_one();
            }
        });
        Ok(())
    }

    /// Stops and waits for the process to exit (force-killed after the stop timeout).
    pub async fn stop_and_wait(self: &Arc<Self>, id: &str, reason: StopReason) -> Result<ServerState> {
        let mut rx = self.runtime(id).state_tx.subscribe();
        self.stop(id, reason)?;
        let state = rx
            .wait_for(|s| !matches!(s, ServerState::Stopping | ServerState::Starting | ServerState::Online))
            .await
            .map(|s| *s)
            .map_err(|_| anyhow!("supervisor closed"))?;
        Ok(state)
    }

    /// Ends the process immediately.
    pub fn kill(&self, id: &str) -> Result<()> {
        let rt = self.runtime(id);
        {
            let mut i = rt.inner.lock().unwrap();
            if !i.state.has_process() {
                bail!("\"{}\" is not running.", i.name);
            }
            if i.requested.is_none() {
                i.requested = Some(StopReason::User);
            }
            i.console.push("[Glasscraft] Force-killing the server.".into());
        }
        rt.kill.notify_one();
        Ok(())
    }

    /// Sends a console command (op, kick, say...).
    pub fn send_command(&self, id: &str, command: &str) -> Result<()> {
        let command = command.trim();
        if command.is_empty() {
            return Ok(());
        }
        let rt = self.runtime(id);
        let mut i = rt.inner.lock().unwrap();
        let Some(tx) = i.stdin.clone().filter(|_| i.state.has_process()) else {
            bail!("\"{}\" is not running.", i.name);
        };
        i.console.push(format!("> {command}"));
        tx.send(command.to_string()).map_err(|_| anyhow!("The server is not accepting commands."))
    }

    pub fn state(&self, id: &str) -> ServerState {
        self.runtimes
            .lock()
            .unwrap()
            .get(id)
            .map(|r| r.inner.lock().unwrap().state)
            .unwrap_or(ServerState::Stopped)
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.state(id).is_active()
    }

    pub fn snapshot(&self, id: &str) -> Snapshot {
        self.runtime(id).snapshot()
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        let list: Vec<Arc<Runtime>> = self.runtimes.lock().unwrap().values().cloned().collect();
        list.iter().map(|r| r.snapshot()).collect()
    }

    pub fn console(&self, id: &str) -> Vec<ConsoleLine> {
        self.runtime(id).inner.lock().unwrap().console.lines()
    }

    pub fn players(&self, id: &str) -> Vec<String> {
        self.runtime(id).inner.lock().unwrap().players.iter().cloned().collect()
    }

    /// Ids of instances that are preparing, starting, online or stopping.
    pub fn active_ids(&self) -> Vec<String> {
        let list: Vec<Arc<Runtime>> = self.runtimes.lock().unwrap().values().cloned().collect();
        list.iter()
            .filter(|r| r.inner.lock().unwrap().state.is_active())
            .map(|r| r.id.clone())
            .collect()
    }

    /// Records a note in the instance's console (shown live and in the log).
    pub fn note(&self, id: &str, text: &str) {
        let rt = self.runtime(id);
        rt.inner.lock().unwrap().console.push(format!("[Glasscraft] {text}"));
        self.flush(&rt);
    }

    pub fn set_notice(&self, id: &str, message: Option<String>) {
        let rt = self.runtime(id);
        self.set_message(&rt, message);
        events::emit(&*self.events, events::INSTANCE_STATE, &rt.snapshot());
    }

    /// Waits until the instance's state satisfies `pred`.
    pub async fn wait_for(&self, id: &str, timeout: Duration, pred: impl Fn(ServerState) -> bool) -> Result<ServerState> {
        let mut rx = self.runtime(id).state_tx.subscribe();
        let result = tokio::time::timeout(timeout, async { rx.wait_for(|s| pred(*s)).await.map(|s| *s) }).await;
        match result {
            Ok(Ok(s)) => Ok(s),
            Ok(Err(_)) => bail!("supervisor closed"),
            Err(_) => bail!("timed out waiting; state is {:?}", self.state(id)),
        }
    }

    /// Gracefully stops every running server (app quit).
    pub async fn stop_all(self: &Arc<Self>, reason: StopReason) {
        let ids: Vec<String> = self
            .active_ids()
            .into_iter()
            .filter(|id| self.state(id).has_process())
            .collect();
        let waits = ids.iter().map(|id| self.stop_and_wait(id, reason));
        futures_util::future::join_all(waits).await;
    }

    fn ensure_metrics_loop(self: &Arc<Self>) {
        if self.metrics_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut sampler = Sampler::new();
            loop {
                tokio::time::sleep(METRICS_INTERVAL).await;
                let Some(sup) = weak.upgrade() else { break };
                let running: Vec<(Arc<Runtime>, u32)> = {
                    let map = sup.runtimes.lock().unwrap();
                    map.values()
                        .filter_map(|r| r.inner.lock().unwrap().pid.map(|p| (r.clone(), p)))
                        .collect()
                };
                if running.is_empty() {
                    continue;
                }
                let pids: Vec<u32> = running.iter().map(|(_, p)| *p).collect();
                let samples = tokio::task::spawn_blocking(move || {
                    let s = sampler.sample(&pids);
                    (sampler, s)
                })
                .await;
                let Ok((back, samples)) = samples else { break };
                sampler = back;
                let mut out = Vec::new();
                for (rt, pid) in running {
                    if let Some((_, m)) = samples.iter().find(|(p, _)| *p == pid) {
                        rt.inner.lock().unwrap().metrics = *m;
                        out.push(rt.snapshot());
                    }
                }
                events::emit(&*sup.events, events::METRICS, &out);
            }
        });
    }
}
