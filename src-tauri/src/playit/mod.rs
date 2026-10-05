//! Fully managed playit.gg (F3, R21–R23): install the agent, link the account once,
//! keep the agent running, and give each instance port a public address.
//!
//! States: not set up → installing → waiting for claim → linked (agent running) /
//! agent offline (restarting). Each port's tunnel is pending → connected, or limit
//! reached / error with a message.

pub mod agent;
pub mod api;
pub mod claim;
pub mod tunnels;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use tokio::sync::Notify;

use crate::core::events::{self, Events};
use crate::core::store::{read_json, write_json_atomic};
use crate::download::{no_progress, Downloader};
use crate::supervisor::Supervisor;
use api::{ApiFailure, ClaimSetup, PlayitApi};

#[derive(Clone)]
pub struct PlayitConfig {
    pub api_base: String,
    pub agent_url: String,
    pub agent_sha256: String,
    /// Runs this program as the agent instead of the downloaded `playitd.exe`
    /// (tests substitute `fake_mc`).
    pub agent_program: Option<PathBuf>,
    pub poll: Duration,
    pub restart_delay: Duration,
    pub tunnel_timeout: Duration,
}

impl Default for PlayitConfig {
    fn default() -> Self {
        Self {
            api_base: api::DEFAULT_API.into(),
            agent_url: agent::AGENT_URL.into(),
            agent_sha256: agent::AGENT_SHA256.into(),
            agent_program: None,
            poll: Duration::from_secs(1),
            restart_delay: Duration::from_secs(3),
            tunnel_timeout: Duration::from_secs(180),
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LinkState {
    NotSetUp,
    Installing,
    WaitingForClaim,
    AgentOffline,
    Linked,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TunnelState {
    Pending,
    Connected,
    LimitReached,
    Error,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub port: u16,
    pub state: TunnelState,
    pub address: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayitStatus {
    pub state: LinkState,
    pub claim_url: Option<String>,
    pub message: Option<String>,
    pub tunnels: Vec<TunnelStatus>,
}

struct Inner {
    state: LinkState,
    claim_url: Option<String>,
    message: Option<String>,
    tunnels: BTreeMap<u16, TunnelStatus>,
    /// Tunnel ids we created, by port (persisted).
    tunnel_ids: BTreeMap<u16, String>,
    in_flight: HashSet<u16>,
    agent_wanted: bool,
    agent_task: bool,
    agent_pid: Option<u32>,
    cancel_claim: Arc<AtomicBool>,
}

pub struct PlayitManager {
    dir: PathBuf,
    api: PlayitApi,
    downloader: Downloader,
    config: PlayitConfig,
    events: Events,
    supervisor: Arc<Supervisor>,
    inner: Mutex<Inner>,
    kill_agent: Notify,
}

impl PlayitManager {
    pub fn new(dir: PathBuf, downloader: Downloader, config: PlayitConfig, events: Events, supervisor: Arc<Supervisor>) -> Arc<Self> {
        let tunnel_ids: BTreeMap<u16, String> = read_json(&dir.join("tunnels.json")).unwrap_or_default();
        let api = PlayitApi::new(downloader.client().clone(), config.api_base.clone());
        let mgr = Arc::new(Self {
            dir,
            api,
            downloader,
            config,
            events,
            supervisor,
            inner: Mutex::new(Inner {
                state: LinkState::NotSetUp,
                claim_url: None,
                message: None,
                tunnels: BTreeMap::new(),
                tunnel_ids,
                in_flight: HashSet::new(),
                agent_wanted: false,
                agent_task: false,
                agent_pid: None,
                cancel_claim: Arc::new(AtomicBool::new(false)),
            }),
            kill_agent: Notify::new(),
        });
        if mgr.secret().is_some() {
            mgr.inner.lock().unwrap().state = LinkState::AgentOffline;
        }
        mgr
    }

    pub fn status(&self) -> PlayitStatus {
        let i = self.inner.lock().unwrap();
        PlayitStatus {
            state: i.state,
            claim_url: i.claim_url.clone(),
            message: i.message.clone(),
            tunnels: i.tunnels.values().cloned().collect(),
        }
    }

    fn emit(&self) {
        events::emit(&*self.events, events::PLAYIT_STATE, &self.status());
    }

    fn set_state(&self, state: LinkState, message: Option<String>) {
        {
            let mut i = self.inner.lock().unwrap();
            i.state = state;
            i.message = message;
            if state != LinkState::WaitingForClaim {
                i.claim_url = None;
            }
        }
        self.emit();
    }

    fn set_tunnel(&self, port: u16, state: TunnelState, address: Option<String>, message: Option<String>) {
        self.inner
            .lock()
            .unwrap()
            .tunnels
            .insert(port, TunnelStatus { port, state, address, message });
        self.emit();
    }

    pub fn secret(&self) -> Option<String> {
        std::fs::read_to_string(agent::secret_path(&self.dir))
            .ok()
            .and_then(|t| claim::parse_secret_file(&t))
    }

    pub fn agent_pid(&self) -> Option<u32> {
        self.inner.lock().unwrap().agent_pid
    }

    pub fn is_linked(&self) -> bool {
        self.secret().is_some()
    }

    /// The public address for a port, once its tunnel is connected.
    pub fn public_address(&self, port: u16) -> Option<String> {
        let i = self.inner.lock().unwrap();
        i.tunnels
            .get(&port)
            .filter(|t| t.state == TunnelState::Connected)
            .and_then(|t| t.address.clone())
    }

    /// On app start: run the agent if the account is already linked.
    pub fn init(self: &Arc<Self>) {
        if self.is_linked() {
            self.start_agent();
        }
    }

    async fn ensure_agent_installed(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config.agent_program {
            return Ok(p.clone());
        }
        agent::install(&self.downloader, &self.config.agent_url, &self.config.agent_sha256, &self.dir, &no_progress).await
    }

    /// Starts the one-time link: installs the agent, then returns the claim URL to
    /// open while approval is awaited in the background.
    pub async fn setup(self: &Arc<Self>) -> Result<String> {
        {
            let i = self.inner.lock().unwrap();
            if let (LinkState::WaitingForClaim, Some(url)) = (i.state, i.claim_url.clone()) {
                return Ok(url);
            }
            if i.state == LinkState::Installing {
                bail!("playit.gg is already being set up.");
            }
        }
        if self.is_linked() {
            bail!("playit.gg is already linked. Use Re-link to link a different account.");
        }
        self.set_state(LinkState::Installing, None);
        if let Err(e) = self.ensure_agent_installed().await {
            let msg = format!("{e:#}");
            self.set_state(LinkState::NotSetUp, Some(msg.clone()));
            return Err(anyhow!(msg));
        }

        let code = claim::generate_code();
        let url = claim::claim_url(&code);
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.inner.lock().unwrap();
            i.state = LinkState::WaitingForClaim;
            i.claim_url = Some(url.clone());
            i.message = Some("Open the link and approve Lodestar on playit.gg.".into());
            i.cancel_claim = cancel.clone();
        }
        self.emit();

        let mgr = self.clone();
        tokio::spawn(async move {
            let version = format!("Lodestar {}", env!("CARGO_PKG_VERSION"));
            let status_mgr = mgr.clone();
            let result = claim::wait_for_secret(&mgr.api, &code, &version, mgr.config.poll, &cancel, move |s| {
                if s == ClaimSetup::WaitingForUser {
                    status_mgr.inner.lock().unwrap().message = Some("Approve Lodestar in your browser to finish.".into());
                    status_mgr.emit();
                }
            })
            .await;
            match result {
                Ok(secret) => {
                    let written = std::fs::create_dir_all(&mgr.dir)
                        .and_then(|_| std::fs::write(agent::secret_path(&mgr.dir), claim::secret_file_contents(&secret)));
                    match written {
                        Ok(()) => {
                            mgr.set_state(LinkState::AgentOffline, Some("Starting the agent…".into()));
                            mgr.start_agent();
                        }
                        Err(e) => mgr.set_state(LinkState::NotSetUp, Some(format!("Could not save the playit.gg key: {e}"))),
                    }
                }
                Err(e) => mgr.set_state(LinkState::NotSetUp, Some(format!("{e:#}"))),
            }
        });
        Ok(url)
    }

    pub fn cancel_setup(&self) {
        let cancel = self.inner.lock().unwrap().cancel_claim.clone();
        cancel.store(true, Ordering::Relaxed);
    }

    /// Unlinks this PC (forgets the key and tunnels) and starts a fresh link.
    pub async fn relink(self: &Arc<Self>) -> Result<String> {
        self.cancel_setup();
        self.stop_agent();
        std::fs::remove_file(agent::secret_path(&self.dir)).ok();
        {
            let mut i = self.inner.lock().unwrap();
            i.tunnels.clear();
            i.tunnel_ids.clear();
            i.state = LinkState::NotSetUp;
        }
        self.save_tunnel_ids();
        self.setup().await
    }

    /// Keeps the agent running, restarting it if it exits on its own.
    pub fn start_agent(self: &Arc<Self>) {
        {
            let mut i = self.inner.lock().unwrap();
            i.agent_wanted = true;
            if i.agent_task {
                return;
            }
            i.agent_task = true;
        }
        let mgr = self.clone();
        tokio::spawn(async move {
            loop {
                if !mgr.inner.lock().unwrap().agent_wanted {
                    break;
                }
                let program = match mgr.ensure_agent_installed().await {
                    Ok(p) => p,
                    Err(e) => {
                        mgr.set_state(LinkState::AgentOffline, Some(format!("{e:#}")));
                        tokio::time::sleep(mgr.config.restart_delay.max(Duration::from_secs(30))).await;
                        continue;
                    }
                };
                match agent::spawn(&program, &mgr.dir, mgr.supervisor.job()) {
                    Ok(mut proc) => {
                        mgr.inner.lock().unwrap().agent_pid = proc.child.id();
                        mgr.set_state(LinkState::Linked, None);
                        tokio::select! {
                            _ = proc.child.wait() => {}
                            _ = mgr.kill_agent.notified() => {
                                let _ = proc.child.kill().await;
                            }
                        }
                    }
                    Err(e) => mgr.set_state(LinkState::AgentOffline, Some(format!("{e:#}"))),
                }
                mgr.inner.lock().unwrap().agent_pid = None;
                if !mgr.inner.lock().unwrap().agent_wanted {
                    break;
                }
                mgr.set_state(LinkState::AgentOffline, Some("The playit.gg agent stopped; restarting it…".into()));
                tokio::time::sleep(mgr.config.restart_delay).await;
            }
            mgr.inner.lock().unwrap().agent_task = false;
        });
    }

    pub fn stop_agent(&self) {
        let running = {
            let mut i = self.inner.lock().unwrap();
            i.agent_wanted = false;
            i.agent_task
        };
        if running {
            self.kill_agent.notify_one();
        }
        if self.is_linked() {
            self.set_state(LinkState::AgentOffline, None);
        }
    }

    fn save_tunnel_ids(&self) {
        let ids = self.inner.lock().unwrap().tunnel_ids.clone();
        let _ = write_json_atomic(&self.dir.join("tunnels.json"), &ids);
    }

    /// Makes sure `port` has a tunnel (once linked), creating it on first use and
    /// waiting in the background for its public address.
    pub fn ensure_tunnel(self: &Arc<Self>, port: u16) {
        if !self.is_linked() {
            return;
        }
        {
            let mut i = self.inner.lock().unwrap();
            if i.tunnels.get(&port).is_some_and(|t| t.state == TunnelState::Connected) || !i.in_flight.insert(port) {
                return;
            }
        }
        self.set_tunnel(port, TunnelState::Pending, None, None);
        let mgr = self.clone();
        tokio::spawn(async move {
            let outcome = mgr.provision_tunnel(port).await;
            mgr.inner.lock().unwrap().in_flight.remove(&port);
            match outcome {
                Ok(address) => mgr.set_tunnel(port, TunnelState::Connected, Some(address), None),
                Err(TunnelError::Limit(m)) => mgr.set_tunnel(port, TunnelState::LimitReached, None, Some(m)),
                Err(TunnelError::Other(m)) => mgr.set_tunnel(port, TunnelState::Error, None, Some(m)),
            }
        });
    }

    async fn provision_tunnel(&self, port: u16) -> std::result::Result<String, TunnelError> {
        let secret = self.secret().ok_or_else(|| TunnelError::Other("playit.gg is not linked.".into()))?;
        let other = |e: anyhow::Error| TunnelError::Other(format!("{e:#}"));
        let known = self.inner.lock().unwrap().tunnel_ids.get(&port).cloned();
        let run = self.api.rundata(&secret).await.map_err(other)?;

        let id = match tunnels::find_for_port(&run, port, known.as_deref()) {
            Some(t) => t.id.clone(),
            None if known.as_deref().is_some_and(|id| tunnels::is_pending(&run, id)) => known.clone().unwrap(),
            None => match self
                .api
                .create_tunnel(&secret, &run.agent_id, port, &tunnels::tunnel_name(port))
                .await
                .map_err(other)?
            {
                Ok(id) => id,
                Err(ApiFailure::Fail(reason)) => {
                    let (limit, msg) = tunnels::describe_create_failure(&reason);
                    return Err(if limit { TunnelError::Limit(msg) } else { TunnelError::Other(msg) });
                }
                Err(e) => return Err(TunnelError::Other(e.to_string())),
            },
        };
        if known.as_deref() != Some(id.as_str()) {
            self.inner.lock().unwrap().tunnel_ids.insert(port, id.clone());
            self.save_tunnel_ids();
        }

        let deadline = tokio::time::Instant::now() + self.config.tunnel_timeout;
        let mut run = run;
        loop {
            if let Some(t) = run.tunnels.iter().find(|t| t.id == id) {
                if let Some(reason) = &t.disabled_reason {
                    return Err(TunnelError::Other(format!("playit.gg disabled the tunnel: {reason}")));
                }
                if !t.display_address.is_empty() {
                    return Ok(t.display_address.clone());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(TunnelError::Other("playit.gg is still setting up the tunnel. Launch again in a minute.".into()));
            }
            tokio::time::sleep(self.config.poll).await;
            run = self.api.rundata(&secret).await.map_err(other)?;
        }
    }

    pub fn shutdown(&self) {
        self.cancel_setup();
        self.stop_agent();
    }
}

enum TunnelError {
    Limit(String),
    Other(String),
}
