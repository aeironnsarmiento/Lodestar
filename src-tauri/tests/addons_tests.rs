mod common;

use std::fs;
use std::io::Write;
use std::time::Duration;

use common::{fixture_server, sha1_hex, test_app};
use lodestar_lib::addons::http::SearchQuery;
use lodestar_lib::addons::{self, AddonSource, ProjectKind};
use lodestar_lib::core::instance::{ModpackRef, NewInstance, Provision, ServerType};

fn fabric_server(app: &lodestar_lib::core::app::App) -> lodestar_lib::core::instance::Instance {
    app.create_instance(NewInstance {
        name: "Modded".into(),
        server_type: ServerType::Fabric,
        mc_version: "26.3".into(),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn files_are_imported_disabled_enabled_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("server").join("mods");
    let drop = dir.path().join("Downloads");
    fs::create_dir_all(&drop).unwrap();
    fs::write(drop.join("cool.jar"), b"cool").unwrap();
    fs::write(drop.join("notes.txt"), b"hi").unwrap();

    let r = addons::import_files(&mods, dir.path(), &[drop.join("cool.jar"), drop.join("notes.txt")]).unwrap();
    assert_eq!(r.added, vec!["cool.jar"]);
    assert_eq!(r.skipped.len(), 1, "only jars are taken");

    addons::set_enabled(&mods, "cool.jar", false).unwrap();
    assert!(mods.join("cool.jar.disabled").exists());
    let listed = addons::list(&mods, dir.path()).unwrap();
    assert_eq!((listed[0].file_name.as_str(), listed[0].enabled), ("cool.jar", false));

    addons::set_enabled(&mods, "cool.jar", true).unwrap();
    addons::remove(&mods, dir.path(), "cool.jar").unwrap();
    assert!(addons::list(&mods, dir.path()).unwrap().is_empty());
    assert!(addons::set_enabled(&mods, "../evil.jar", true).is_err(), "names cannot leave the folder");
}

#[test]
fn modpack_paths_cannot_escape_the_server_folder() {
    assert!(addons::safe_relative("mods/a.jar").is_ok());
    for bad in ["../a.jar", "/etc/passwd", "C:\\Windows\\a.jar", "mods/../../a.jar", ""] {
        assert!(addons::safe_relative(bad).is_err(), "{bad}");
    }
}

#[tokio::test]
async fn installing_from_modrinth_brings_required_dependencies_and_updates() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = fabric_server(&app);

    // An older ServerCore dropped in by hand is identified by its hash.
    let dropped = dir.path().join("servercore-1.1.0.jar");
    fs::write(&dropped, b"servercore 1.1").unwrap();
    app.import_addons(&inst.id, &[dropped]).unwrap();
    let listed = app.identify_addons(&inst.id).await.unwrap();
    assert_eq!(listed[0].meta.title.as_deref(), Some("ServerCore"));
    assert_eq!(listed[0].meta.version_id.as_deref(), Some("sc-1"));

    let updates = app.check_addon_updates(&inst.id).await.unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].version_id, "sc-2");

    let result = app.update_addon(&inst.id, "servercore-1.1.0.jar", "sc-2").await.unwrap();
    assert_eq!(result.installed, vec!["ServerCore", "Fabric API"], "the dependency comes along");
    let names: Vec<String> = app.list_addons(&inst.id).unwrap().into_iter().map(|e| e.file_name).collect();
    assert!(names.contains(&"servercore-1.2.0.jar".to_string()));
    assert!(!names.contains(&"servercore-1.1.0.jar".to_string()), "the old version is replaced");
    assert!(names.contains(&"fabric-api-0.100.0.jar".to_string()));

    // Installing again does not reinstall the dependency.
    let again = app.install_project(&inst.id, AddonSource::Modrinth, "servercore", None).await.unwrap();
    assert_eq!(again.installed, vec!["ServerCore"]);
}

#[tokio::test]
async fn curseforge_needs_an_api_key() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let err = app
        .search_projects(AddonSource::Curseforge, ProjectKind::Mod, SearchQuery::default())
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("CurseForge API key"));
}

fn write_mrpack(path: &std::path::Path, base: &str) {
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": "Test Pack",
        "dependencies": { "minecraft": "26.3", "fabric-loader": "0.19.5" },
        "files": [
            {
                "path": "mods/pack-mod.jar",
                "hashes": { "sha1": sha1_hex(b"pack mod") },
                "env": { "client": "required", "server": "required" },
                "downloads": [format!("{base}/files/pack-mod.jar")],
            },
            {
                "path": "mods/client-only.jar",
                "hashes": { "sha1": "00" },
                "env": { "client": "required", "server": "unsupported" },
                "downloads": [format!("{base}/files/missing.jar")],
            },
        ],
    });
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    z.start_file("modrinth.index.json", opts).unwrap();
    z.write_all(index.to_string().as_bytes()).unwrap();
    z.start_file("overrides/config/pack.toml", opts).unwrap();
    z.write_all(b"pack = true").unwrap();
    z.start_file("server-overrides/server-only.txt", opts).unwrap();
    z.write_all(b"server").unwrap();
    z.finish().unwrap();
}

#[tokio::test]
async fn a_modpack_file_becomes_a_server_with_its_mods_and_settings() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let pack = dir.path().join("test.mrpack");
    write_mrpack(&pack, &server.base);

    let info = app.inspect_modpack_file(&pack).unwrap();
    assert_eq!((info.name.as_str(), info.mc_version.as_str(), info.server_type), ("Test Pack", "26.3", ServerType::Fabric));
    assert_eq!(info.loader_version.as_deref(), Some("0.19.5"));

    let inst = app
        .create_and_provision(NewInstance {
            name: "Pack server".into(),
            server_type: info.server_type,
            mc_version: info.mc_version.clone(),
            loader_version: info.loader_version.clone(),
            modpack: Some(ModpackRef {
                title: info.name.clone(),
                file: Some(pack.to_string_lossy().into_owned()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
    assert!(!inst.speed_mods, "packs bring their own performance mods");

    let mut state = Provision::Pending;
    for _ in 0..400 {
        state = app.store.get(&inst.id).unwrap().provision;
        if matches!(state, Provision::Ready | Provision::Failed { .. }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(state, Provision::Ready);

    let server_dir = app.paths().server_dir(&inst.id);
    assert_eq!(fs::read(server_dir.join("mods/pack-mod.jar")).unwrap(), b"pack mod");
    assert!(!server_dir.join("mods/client-only.jar").exists(), "client-only files are skipped");
    assert_eq!(fs::read_to_string(server_dir.join("config/pack.toml")).unwrap(), "pack = true");
    assert!(server_dir.join("server-only.txt").exists());

    let after = app.store.get(&inst.id).unwrap();
    assert!(after.modpack.as_ref().unwrap().installed);
    assert_eq!(after.loader_version.as_deref(), Some("0.19.5"));
    let listed = app.list_addons(&inst.id).unwrap();
    assert!(listed.iter().any(|e| e.file_name == "pack-mod.jar" && e.meta.from_modpack));
}
