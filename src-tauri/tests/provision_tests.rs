mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::{fixture_server, TestServer};
use lodestar_lib::core::app::{App, AppConfig};
use lodestar_lib::core::events::MemorySink;
use lodestar_lib::core::instance::{Instance, LaunchInfo, NewInstance, Provision, ServerType};
use lodestar_lib::core::paths::Paths;
use lodestar_lib::providers::Endpoints;
use lodestar_lib::supervisor::{ServerState, StopReason};
use lodestar_lib::worlds::properties::read_value;

const WAIT: Duration = Duration::from_secs(10);

fn app(server: &TestServer, root: &std::path::Path) -> Arc<App> {
    let mut config = AppConfig::new(Paths::new(root));
    config.endpoints = Endpoints::local(&server.base);
    config.adoptium_api = format!("{}/adoptium", server.base);
    config.download_backoff = Duration::from_millis(5);
    config.java_override = Some(PathBuf::from(env!("CARGO_BIN_EXE_fake_mc")));
    App::new(config, MemorySink::new()).unwrap()
}

fn fabric(name: &str, version: &str) -> NewInstance {
    NewInstance {
        name: name.into(),
        server_type: ServerType::Fabric,
        mc_version: version.into(),
        seed: Some("speedrun123".into()),
        ..Default::default()
    }
}

/// A port nothing listens on, so tests never collide with a real server on 25565.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn wait_provisioned(app: &App, id: &str) -> Instance {
    for _ in 0..400 {
        let inst = app.store.get(id).unwrap();
        if matches!(inst.provision, Provision::Ready | Provision::Failed { .. }) {
            return inst;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("provisioning did not finish");
}

#[tokio::test]
async fn a_fabric_26_3_instance_provisions_java_server_files_and_speed_mods() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());

    let created = app.create_and_provision(fabric("Speedrun", "26.3")).unwrap();
    assert!(matches!(created.provision, Provision::Pending));
    assert!(matches!(app.store.get(&created.id).unwrap().provision, Provision::Running { .. }));

    let inst = wait_provisioned(&app, &created.id).await;
    assert_eq!(inst.provision, Provision::Ready);
    assert_eq!(inst.java_major, Some(25));
    assert!(matches!(&inst.launch, Some(LaunchInfo::Jar { jar }) if jar.contains("fabric-26.3")));
    assert_eq!(inst.managed_mods.len(), 1, "lithium installs; ferrite-core has no build and is skipped");
    assert_eq!(inst.managed_mods_for.as_deref(), Some("fabric-26.3"));
    assert!(lodestar_lib::java::java_binary(&Paths::new(dir.path()).java_dir(25)).is_file());
    assert!(Paths::new(dir.path()).server_dir(&inst.id).join("mods").join(&inst.managed_mods[0]).is_file());
}

#[tokio::test]
async fn a_provisioning_failure_is_reported_blocks_launch_and_can_be_retried() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());
    app.accept_eula().unwrap();

    // 99.9 is not in Mojang's manifest.
    let inst = app.create_and_provision(fabric("Broken", "99.9")).unwrap();
    let inst = wait_provisioned(&app, &inst.id).await;
    let Provision::Failed { message } = &inst.provision else { panic!("expected failure") };
    assert!(message.contains("99.9"), "{message}");

    let err = app.launch(&inst.id).await.unwrap_err().to_string();
    assert!(err.contains("could not be set up"), "{err}");
    assert_eq!(app.supervisor.state(&inst.id), ServerState::Stopped);

    // Fix the version and retry.
    let mut fixed = inst.clone();
    fixed.mc_version = "26.3".into();
    app.store.update(fixed).unwrap();
    app.retry_provision(&inst.id).unwrap();
    assert_eq!(wait_provisioned(&app, &inst.id).await.provision, Provision::Ready);
}

#[tokio::test]
async fn launching_without_the_eula_is_refused_and_writes_no_eula_file() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());
    let inst = app.create_and_provision(fabric("NoEula", "26.3")).unwrap();
    wait_provisioned(&app, &inst.id).await;

    let err = app.launch(&inst.id).await.unwrap_err().to_string();
    assert!(err.contains("EULA"), "{err}");
    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    assert!(!server_dir.join("eula.txt").exists());
    assert!(!server_dir.join("server.properties").exists());
    assert_eq!(app.supervisor.state(&inst.id), ServerState::Stopped);
}

#[tokio::test]
async fn launching_after_eula_writes_eula_and_properties_before_spawning() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());
    let inst = app.create_and_provision(fabric("Run", "26.3")).unwrap();
    wait_provisioned(&app, &inst.id).await;
    let mut inst = app.store.get(&inst.id).unwrap();
    inst.max_players = 4;
    inst.motd = "Speedrun night".into();
    inst.hardcore = true;
    inst.port = free_port();
    let inst = app.update_instance(inst).unwrap();

    app.accept_eula().unwrap();
    assert!(app.settings().eula_accepted_at.is_some());
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    let eula = std::fs::read_to_string(server_dir.join("eula.txt")).unwrap();
    assert!(eula.lines().any(|l| l == "eula=true"));
    assert_eq!(read_value(&server_dir, "server-port").unwrap(), inst.port.to_string());
    assert_eq!(read_value(&server_dir, "max-players").unwrap(), "4");
    assert_eq!(read_value(&server_dir, "motd").unwrap(), "Speedrun night");
    assert_eq!(read_value(&server_dir, "hardcore").unwrap(), "true");
    assert_eq!(read_value(&server_dir, "level-seed").unwrap(), "speedrun123");
    // fake_mc read the properties written before it was spawned.
    assert!(app.console(&inst.id).iter().any(|l| l.text.contains("Using seed speedrun123")));

    // Live stats and stop.
    assert!(app.snapshots().iter().any(|s| s.id == inst.id && s.state == ServerState::Online));
    app.supervisor.stop_and_wait(&inst.id, StopReason::User).await.unwrap();
    assert_eq!(app.supervisor.state(&inst.id), ServerState::Stopped);
}

#[tokio::test]
async fn deleting_a_running_instance_is_refused_through_the_app() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());
    app.accept_eula().unwrap();
    let inst = app.create_and_provision(fabric("Busy", "26.3")).unwrap();
    wait_provisioned(&app, &inst.id).await;
    app.store.modify(&inst.id, |i| i.port = free_port()).unwrap();
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    assert!(app.delete_instance(&inst.id).unwrap_err().to_string().contains("Stop"));
    app.supervisor.stop_and_wait(&inst.id, StopReason::User).await.unwrap();
    app.delete_instance(&inst.id).unwrap();
}

#[tokio::test]
async fn addons_folder_exists_for_modded_types_only() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = app(&server, dir.path());
    let fab = app.create_instance(fabric("Mods", "26.3")).unwrap();
    assert!(app.addons_folder(&fab.id).unwrap().ends_with("mods"));
    let paper = app
        .create_instance(NewInstance { name: "P".into(), server_type: ServerType::Paper, mc_version: "26.3".into(), ..Default::default() })
        .unwrap();
    assert!(app.addons_folder(&paper.id).unwrap().ends_with("plugins"));
    let vanilla = app
        .create_instance(NewInstance { name: "V".into(), server_type: ServerType::Vanilla, mc_version: "26.3".into(), ..Default::default() })
        .unwrap();
    assert!(app.addons_folder(&vanilla.id).is_err());
}
