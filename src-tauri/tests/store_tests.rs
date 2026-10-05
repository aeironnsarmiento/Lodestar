use std::fs;

use glasscraft_lib::core::instance::{NewInstance, ServerType};
use glasscraft_lib::core::paths::Paths;
use glasscraft_lib::core::settings::{AppSettings, Theme};
use glasscraft_lib::core::store::{write_json_atomic, Store};

fn new_instance(name: &str) -> NewInstance {
    NewInstance {
        name: name.into(),
        server_type: ServerType::Fabric,
        mc_version: "26.3".into(),
        ..Default::default()
    }
}

#[test]
fn creating_an_instance_writes_instance_json_and_lists_it() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Store::open(paths.clone()).unwrap();

    let inst = store.create(new_instance("Speedrun Server")).unwrap();
    assert_eq!(inst.id, "speedrun-server");
    assert!(paths.instance_file(&inst.id).is_file());
    assert!(paths.server_dir(&inst.id).is_dir());

    let listed = store.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Speedrun Server");

    // A fresh store reads it back from disk.
    let reopened = Store::open(paths).unwrap();
    assert_eq!(reopened.get(&inst.id).unwrap().mc_version, "26.3");
}

#[test]
fn settings_persist_across_store_instances() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Store::open(paths.clone()).unwrap();
    assert_eq!(store.settings(), AppSettings::default());

    store
        .modify_settings(|s| {
            s.theme = Theme::Light;
            s.reduce_effects = true;
            s.eula_accepted_at = Some("2026-10-05T10:00:00+00:00".into());
        })
        .unwrap();

    let reopened = Store::open(paths).unwrap();
    let s = reopened.settings();
    assert_eq!(s.theme, Theme::Light);
    assert!(s.reduce_effects);
    assert!(s.eula_accepted_at.is_some());
}

#[test]
fn same_name_instances_get_distinct_folders_and_ports() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Paths::new(dir.path())).unwrap();
    let a = store.create(new_instance("Practice")).unwrap();
    let b = store.create(new_instance("Practice")).unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(b.id, "practice-2");
    assert_ne!(a.port, b.port);
}

#[test]
fn corrupt_instance_json_is_reported_and_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Store::open(paths.clone()).unwrap();
    let good = store.create(new_instance("Good")).unwrap();
    let bad = store.create(new_instance("Bad")).unwrap();
    // Simulate a half-written file.
    fs::write(paths.instance_file(&bad.id), b"{\"id\": \"bad\", \"name\": \"Ba").unwrap();

    let reopened = Store::open(paths).unwrap();
    let ids: Vec<String> = reopened.list().into_iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![good.id]);
    assert_eq!(reopened.load_errors().len(), 1);
    assert!(reopened.load_errors()[0].contains("instance.json"));
}

#[test]
fn deleting_a_running_instance_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Store::open(paths.clone()).unwrap();
    let inst = store.create(new_instance("Live")).unwrap();

    let err = store.delete(&inst.id, true).unwrap_err().to_string();
    assert!(err.contains("Stop \"Live\""), "{err}");
    assert!(paths.instance_dir(&inst.id).exists());

    store.delete(&inst.id, false).unwrap();
    assert!(!paths.instance_dir(&inst.id).exists());
    assert!(store.list().is_empty());
}

#[test]
fn interrupted_atomic_write_keeps_the_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let store = Store::open(paths.clone()).unwrap();
    let inst = store.create(new_instance("Safe")).unwrap();

    // A crash between writing the temp file and renaming it leaves the temp behind.
    let file = paths.instance_file(&inst.id);
    let tmp = file.with_file_name("instance.json.tmp");
    fs::write(&tmp, b"{ partial").unwrap();

    let reopened = Store::open(paths.clone()).unwrap();
    assert_eq!(reopened.get(&inst.id).unwrap().name, "Safe");
    assert!(reopened.load_errors().is_empty());

    // The next write replaces the stale temp file cleanly.
    let mut changed = reopened.get(&inst.id).unwrap();
    changed.motd = "updated".into();
    reopened.update(changed).unwrap();
    assert!(!tmp.exists());
    assert_eq!(Store::open(paths).unwrap().get(&inst.id).unwrap().motd, "updated");

    // And the helper itself never leaves a temp file on success.
    let other = dir.path().join("x.json");
    write_json_atomic(&other, &serde_json::json!({"a": 1})).unwrap();
    assert!(!dir.path().join("x.json.tmp").exists());
}

#[test]
fn empty_names_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Paths::new(dir.path())).unwrap();
    assert!(store.create(new_instance("   ")).is_err());
}
