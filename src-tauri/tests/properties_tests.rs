use std::fs;

use lodestar_lib::core::instance::{clean_player_list, Difficulty, Instance, LevelType, NewInstance, ServerType};
use lodestar_lib::core::paths::Paths;
use lodestar_lib::core::store::Store;
use lodestar_lib::worlds::properties::{read_value, remove_from_player_file, write_server_properties};

#[test]
fn every_server_property_from_the_app_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut inst = Instance::default();
    inst.properties.white_list = true;
    inst.properties.enforce_whitelist = true;
    inst.properties.pvp = false;
    inst.properties.level_type = LevelType::LargeBiomes;
    inst.properties.player_idle_timeout = 15;
    inst.properties.resource_pack = "https://example.com/pack.zip".into();
    write_server_properties(dir.path(), &inst, "123").unwrap();

    for (key, value) in [
        ("white-list", "true"),
        ("enforce-whitelist", "true"),
        ("pvp", "false"),
        ("level-type", "largebiomes"),
        ("player-idle-timeout", "15"),
        ("spawn-protection", "0"),
        ("allow-flight", "true"),
        ("resource-pack", "https://example.com/pack.zip"),
    ] {
        assert_eq!(read_value(dir.path(), key).as_deref(), Some(value), "{key}");
    }
}

#[test]
fn player_lists_are_cleaned_and_values_kept_in_range() {
    let names = ["Steve", " alex ", "steve", "bad name", "", "Way_Too_Long_Name_123"].map(String::from);
    assert_eq!(clean_player_list(&names), vec!["Steve", "alex"]);

    let mut inst = Instance { hardcore: true, difficulty: Difficulty::Easy, ..Default::default() };
    inst.properties.entity_broadcast_range_percentage = 5;
    inst.properties.player_idle_timeout = 99_999;
    inst.normalize();
    assert_eq!(inst.difficulty, Difficulty::Hard, "hardcore always plays on hard");
    assert_eq!(inst.properties.entity_broadcast_range_percentage, 10);
    assert_eq!(inst.properties.player_idle_timeout, 1440);
}

#[test]
fn a_hardcore_server_can_be_created() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Paths::new(dir.path())).unwrap();
    let inst = store
        .create(NewInstance {
            name: "Hard".into(),
            server_type: ServerType::Vanilla,
            mc_version: "26.3".into(),
            hardcore: true,
            ..Default::default()
        })
        .unwrap();
    assert!(inst.hardcore);
    assert_eq!(inst.difficulty, Difficulty::Hard);
    // New servers start with 8 GB, 16 view distance and 8 simulation distance.
    assert_eq!((inst.ram_mb, inst.view_distance, inst.simulation_distance), (8192, 16, 8));
}

#[test]
fn removing_players_edits_the_list_file_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whitelist.json");
    fs::write(&path, r#"[{"uuid":"1","name":"Steve"},{"uuid":"2","name":"Alex"}]"#).unwrap();
    remove_from_player_file(&path, &["steve".into()]).unwrap();
    let left: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["name"], "Alex");

    // A missing file is not an error.
    remove_from_player_file(&dir.path().join("ops.json"), &["Steve".into()]).unwrap();
}
