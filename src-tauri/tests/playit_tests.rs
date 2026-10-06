mod common;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{fixture, Response, TestServer};
use lodestar_lib::core::events::MemorySink;
use lodestar_lib::download::Downloader;
use lodestar_lib::playit::agent::{agent_path, secret_path};
use lodestar_lib::playit::claim::{parse_secret_file, secret_file_contents};
use lodestar_lib::playit::{LinkState, PlayitConfig, PlayitManager, TunnelState};
use lodestar_lib::supervisor::job_object::process_alive;
use lodestar_lib::supervisor::Supervisor;

const SECRET: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

/// A playit.gg stand-in: each path answers from a queue of fixtures; the last one
/// repeats.
struct Script(Mutex<HashMap<String, VecDeque<String>>>);

impl Script {
    fn new(routes: &[(&str, &[&str])]) -> Arc<Self> {
        Arc::new(Self(Mutex::new(
            routes
                .iter()
                .map(|(p, names)| (p.to_string(), names.iter().map(|s| s.to_string()).collect()))
                .collect(),
        )))
    }

    fn respond(&self, path: &str) -> Response {
        let mut map = self.0.lock().unwrap();
        let Some(queue) = map.get_mut(path) else { return Response::status(404) };
        let name = if queue.len() > 1 { queue.pop_front().unwrap() } else { queue[0].clone() };
        Response::ok(fixture(&format!("playit/{name}.json")))
    }
}

fn playit_server(script: Arc<Script>) -> TestServer {
    TestServer::start(move |req| {
        if req.path == "/agent.exe" {
            return Response::ok(b"definitely not the signed agent".to_vec());
        }
        script.respond(&req.path)
    })
}

fn manager(server: &TestServer, dir: &Path, agent: Option<PathBuf>) -> Arc<PlayitManager> {
    let sink = MemorySink::new();
    let config = PlayitConfig {
        api_base: server.base.clone(),
        agent_url: format!("{}/agent.exe", server.base),
        agent_program: agent,
        poll: Duration::from_millis(10),
        restart_delay: Duration::from_millis(100),
        tunnel_timeout: Duration::from_secs(3),
        ..PlayitConfig::default()
    };
    PlayitManager::new(
        dir.to_path_buf(),
        Downloader::new().with_backoff(Duration::from_millis(5)),
        config,
        sink.clone(),
        Supervisor::new(sink),
    )
}

fn fake_agent() -> Option<PathBuf> {
    Some(PathBuf::from(env!("CARGO_BIN_EXE_fake_mc")))
}

fn linked(dir: &Path) {
    std::fs::write(secret_path(dir), secret_file_contents(SECRET)).unwrap();
}

async fn eventually(mut check: impl FnMut() -> bool) {
    for _ in 0..300 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not met in time");
}

#[tokio::test]
async fn the_claim_sequence_stores_the_secret_and_links_the_agent() {
    let script = Script::new(&[
        ("/claim/setup", &["claim_setup_WaitingForUserVisit", "claim_setup_WaitingForUser", "claim_setup_UserAccepted"]),
        ("/claim/exchange", &["claim_exchange_not_accepted", "claim_exchange_ok"]),
    ]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(&server, dir.path(), fake_agent());
    assert_eq!(mgr.status().state, LinkState::NotSetUp);

    let url = mgr.setup().await.unwrap();
    assert!(url.starts_with("https://playit.gg/claim/"));
    let code = url.rsplit('/').next().unwrap().to_string();
    assert_eq!(code.len(), 10);
    assert_eq!(mgr.status().state, LinkState::WaitingForClaim);
    assert_eq!(mgr.status().claim_url.as_deref(), Some(url.as_str()));

    eventually(|| mgr.status().state == LinkState::Linked).await;
    let stored = std::fs::read_to_string(secret_path(dir.path())).unwrap();
    assert_eq!(parse_secret_file(&stored).as_deref(), Some(SECRET));

    let setup_calls: Vec<serde_json::Value> = server.requests().iter().filter(|r| r.path == "/claim/setup").map(|r| r.json()).collect();
    assert!(setup_calls.len() >= 3);
    assert_eq!(setup_calls[0]["code"], code);
    assert_eq!(setup_calls[0]["agent_type"], "self-managed");
    assert!(server.requests().iter().all(|r| r.header("authorization").is_none()), "claim calls are unauthenticated");
    mgr.shutdown();
}

#[tokio::test]
async fn a_rejected_claim_returns_to_not_set_up_with_a_message() {
    let script = Script::new(&[("/claim/setup", &["claim_setup_WaitingForUser", "claim_setup_UserRejected"])]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(&server, dir.path(), fake_agent());
    mgr.setup().await.unwrap();
    eventually(|| mgr.status().state == LinkState::NotSetUp).await;
    assert!(mgr.status().message.unwrap().contains("declined"));
    assert!(!secret_path(dir.path()).exists());
}

#[tokio::test]
async fn a_new_port_gets_a_tunnel_created_then_connected() {
    let script = Script::new(&[
        ("/v1/agents/rundata", &["rundata_empty", "rundata_pending", "rundata_no_address", "rundata_connected"]),
        ("/tunnels/create", &["tunnels_create_ok"]),
    ]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    linked(dir.path());
    let mgr = manager(&server, dir.path(), fake_agent());

    mgr.ensure_tunnel(25565);
    assert_eq!(mgr.status().tunnels[0].state, TunnelState::Pending);
    eventually(|| mgr.public_address(25565).is_some()).await;
    assert_eq!(mgr.public_address(25565).as_deref(), Some("glass-runs.gl.joinmc.link"));
    assert_eq!(mgr.status().tunnels[0].state, TunnelState::Connected);

    let creates: Vec<_> = server.requests().into_iter().filter(|r| r.path == "/tunnels/create").collect();
    assert_eq!(creates.len(), 1);
    let body = creates[0].json();
    // The shape the live API accepts (checked against api.playit.gg on 2026-10-05).
    assert_eq!(body["tunnel_type"], "minecraft-java");
    assert_eq!(body["port_type"], "tcp");
    assert_eq!(body["port_count"], 1);
    assert_eq!(body["origin"]["type"], "agent");
    assert_eq!(body["origin"]["data"]["agent_id"], "5d6b2f40-9b0c-4c63-9f3c-2a7d1e9c8b11");
    assert_eq!(body["origin"]["data"]["local_ip"], "127.0.0.1");
    assert_eq!(body["origin"]["data"]["local_port"], 25565);
    assert_eq!(creates[0].header("authorization"), Some(format!("Agent-Key {SECRET}").as_str()));
    // The tunnel id is remembered for next time.
    assert!(std::fs::read_to_string(dir.path().join("tunnels.json")).unwrap().contains("8a1c3e55"));
}

#[tokio::test]
async fn an_existing_tunnel_for_the_port_is_reused() {
    let script = Script::new(&[("/v1/agents/rundata", &["rundata_connected"]), ("/tunnels/create", &["tunnels_create_ok"])]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    linked(dir.path());
    let mgr = manager(&server, dir.path(), fake_agent());
    mgr.ensure_tunnel(25565);
    eventually(|| mgr.public_address(25565).is_some()).await;
    assert_eq!(server.count("/tunnels/create"), 0);
}

#[tokio::test]
async fn two_instances_on_one_port_share_one_tunnel() {
    let script = Script::new(&[
        ("/v1/agents/rundata", &["rundata_empty", "rundata_no_address", "rundata_connected"]),
        ("/tunnels/create", &["tunnels_create_ok"]),
    ]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    linked(dir.path());
    let mgr = manager(&server, dir.path(), fake_agent());
    mgr.ensure_tunnel(25565);
    mgr.ensure_tunnel(25565);
    eventually(|| mgr.public_address(25565).is_some()).await;
    mgr.ensure_tunnel(25565);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.count("/tunnels/create"), 1);
    assert_eq!(mgr.status().tunnels.len(), 1);
}

#[tokio::test]
async fn a_tunnel_limit_sets_limit_reached_with_a_clear_message() {
    let script = Script::new(&[("/v1/agents/rundata", &["rundata_empty"]), ("/tunnels/create", &["tunnels_create_limit"])]);
    let server = playit_server(script);
    let dir = tempfile::tempdir().unwrap();
    linked(dir.path());
    let mgr = manager(&server, dir.path(), fake_agent());
    mgr.ensure_tunnel(25566);
    eventually(|| mgr.status().tunnels.first().is_some_and(|t| t.state == TunnelState::LimitReached)).await;
    let t = &mgr.status().tunnels[0];
    assert!(t.message.as_deref().unwrap().contains("no free tunnel"));
    assert_eq!(mgr.public_address(25566), None);
}

#[tokio::test]
async fn an_agent_with_the_wrong_hash_is_refused_and_deleted() {
    let server = playit_server(Script::new(&[]));
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(&server, dir.path(), None);
    let err = mgr.setup().await.unwrap_err();
    assert!(format!("{err:#}").contains("SHA-256"), "{err:#}");
    let agent = agent_path(dir.path());
    assert!(!agent.exists());
    assert!(!agent.with_file_name(format!("{}.part", agent.file_name().unwrap().to_string_lossy())).exists());
    assert_eq!(mgr.status().state, LinkState::NotSetUp);
    assert!(mgr.status().message.is_some());
}

#[tokio::test]
async fn an_agent_that_exits_is_restarted_and_shows_offline_meanwhile() {
    let server = playit_server(Script::new(&[]));
    let dir = tempfile::tempdir().unwrap();
    linked(dir.path());
    let mgr = manager(&server, dir.path(), fake_agent());
    assert_eq!(mgr.status().state, LinkState::AgentOffline, "linked but not running yet");
    mgr.init();
    eventually(|| mgr.status().state == LinkState::Linked && mgr.agent_pid().is_some()).await;
    let first = mgr.agent_pid().unwrap();

    if cfg!(windows) {
        std::process::Command::new("taskkill").args(["/F", "/PID", &first.to_string()]).output().unwrap();
    } else {
        std::process::Command::new("kill").args(["-9", &first.to_string()]).output().unwrap();
    }
    eventually(|| mgr.status().state == LinkState::AgentOffline).await;
    assert!(mgr.status().message.unwrap().contains("restarting"));
    eventually(|| mgr.status().state == LinkState::Linked && mgr.agent_pid().is_some_and(|p| p != first)).await;

    let second = mgr.agent_pid().unwrap();
    mgr.shutdown();
    eventually(|| !process_alive(second)).await;
}

/// Opt-in check against the real services: `LODESTAR_LIVE_TESTS=1 cargo test live_`.
/// Downloads the pinned agent (verifying its SHA-256) and asks playit.gg for the state
/// of a fresh claim code. Nothing is claimed and no account is created.
#[tokio::test]
async fn live_agent_hash_and_claim_api_match() {
    if std::env::var("LODESTAR_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipped: set LODESTAR_LIVE_TESTS=1 to run");
        return;
    }
    use lodestar_lib::playit::api::{ClaimSetup, PlayitApi, DEFAULT_API};
    use lodestar_lib::playit::{agent, claim};
    let dir = tempfile::tempdir().unwrap();
    let d = Downloader::new();
    // Only Windows downloads the agent; macOS bundles one built from source.
    if !agent::AGENT_URL.is_empty() {
        let exe = agent::install(&d, agent::AGENT_URL, agent::AGENT_SHA256, dir.path(), &lodestar_lib::download::no_progress)
            .await
            .unwrap();
        assert!(exe.is_file());
    }

    let api = PlayitApi::new(d.client().clone(), DEFAULT_API);
    let status = api.claim_setup(&claim::generate_code(), "Lodestar live test").await.unwrap().unwrap();
    assert_eq!(status, ClaimSetup::WaitingForUserVisit);
}
