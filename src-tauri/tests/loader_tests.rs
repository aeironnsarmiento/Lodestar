mod common;

use std::io::Write;
use std::path::Path;

use common::fixture;
use lodestar_lib::addons::requirements::{self, parse_mods_toml, range_allows, summarize};
use lodestar_lib::core::instance::{LaunchInfo, ServerType};
use lodestar_lib::core::loader::{accepts, choose};
use lodestar_lib::providers::{fabric, forge, loader_from_launch, neoforge, LoaderTag};

const TSP_TOML: &str = r#"
modLoader="javafml"
loaderVersion="[47,)"
license="All rights reserved"

[[mods]]
modId="tsp"
version="3.4"
displayName="§6§lTizio Space Project§r"
description='''
A long description.
[[dependencies.fake]]
modId="forge"
'''

[[dependencies.tsp]]
    modId="forge"
    mandatory=true
    versionRange="[47.4.20,)" # needs the newer event bus
    ordering="NONE"
    side="BOTH"

[[dependencies.tsp]]
    modId="minecraft"
    mandatory=true
    versionRange="[1.20.1,1.21)"

[[dependencies.tsp]]
    modId="forge"
    mandatory=false
    versionRange="[99,)"
"#;

#[test]
fn mods_toml_yields_required_loader_ranges_with_display_names() {
    assert_eq!(parse_mods_toml(TSP_TOML, "forge"), vec![("Tizio Space Project".to_string(), "[47.4.20,)".to_string())]);
    // NeoForge's `type = "required"` form, and an owner missing from [[mods]].
    let neo = "[[mods]]\nmodId=\"a\"\n[[dependencies.b]]\nmodId=\"neoforge\"\ntype=\"required\"\nversionRange=\"[21.1.100,)\"\n\
               [[dependencies.b]]\nmodId=\"neoforge\"\ntype=\"optional\"\nversionRange=\"[30,)\"\n";
    assert_eq!(parse_mods_toml(neo, "neoforge"), vec![("a".to_string(), "[21.1.100,)".to_string())]);
}

#[test]
fn maven_ranges_are_checked_numerically() {
    assert!(!range_allows("[47.4.20,)", "47.4.10"));
    assert!(range_allows("[47.4.20,)", "47.4.20"));
    assert!(range_allows("[47.4.20,)", "47.4.26"));
    assert!(range_allows("[47,48)", "47.4.10"));
    assert!(!range_allows("[47,48)", "48.0.1"));
    assert!(!range_allows("(47.1.0,47.2]", "47.1.0"));
    assert!(range_allows("(47.1.0,47.2]", "47.2"));
    assert!(range_allows("[47.1.3]", "47.1.3"));
    assert!(!range_allows("[47.1.3]", "47.1.4"));
    assert!(range_allows("[46,47),[47.2,)", "47.3.0"));
    assert!(!range_allows("[46,47),[47.2,)", "47.1.0"));
    // Bare versions and placeholders accept anything.
    assert!(range_allows("47.4.20", "1.0"));
    assert!(range_allows("${forge_range}", "1.0"));
    assert_eq!(summarize("[47.4.20,)"), "47.4.20 or newer");
    assert_eq!(summarize("[47,48)"), "47 up to (not including) 48");
}

fn write_jar(path: &Path, entry: &str, text: &str) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file(entry, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(text.as_bytes()).unwrap();
    zip.finish().unwrap();
}

#[test]
fn scanning_a_mods_folder_reports_bounded_requirements_of_enabled_jars() {
    let dir = tempfile::tempdir().unwrap();
    write_jar(&dir.path().join("TizioSpaceProject.jar"), "META-INF/mods.toml", TSP_TOML);
    write_jar(&dir.path().join("loose.jar"), "META-INF/mods.toml", "[[dependencies.x]]\nmodId=\"forge\"\nmandatory=true\nversionRange=\"[0,)\"\n");
    write_jar(&dir.path().join("off.jar.disabled"), "META-INF/mods.toml", TSP_TOML);
    std::fs::write(dir.path().join("broken.jar"), b"not a zip").unwrap();

    let reqs = requirements::scan(dir.path(), ServerType::Forge);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].file_name, "TizioSpaceProject.jar");
    assert_eq!(reqs[0].summary, "47.4.20 or newer");
    assert!(requirements::scan(dir.path(), ServerType::Fabric).is_empty());
}

#[test]
fn forge_builds_are_listed_newest_first_and_automatic_honours_mod_requirements() {
    let mut meta = forge::parse_maven_metadata(&fixture("providers/forge_maven_metadata.xml"));
    // The trimmed fixture lacks the recommended 1.20.1 build.
    meta.push(("1.20.1".into(), "1.20.1-47.4.10".into()));
    let promos = forge::parse_promotions(&fixture("providers/forge_promotions_slim.json")).unwrap();
    let builds = forge::builds_for(&meta, &promos, "1.20.1");
    assert_eq!(builds[0].id, "47.4.26");
    assert_eq!(builds[0].tag, Some(LoaderTag::Latest));
    let rec = builds.iter().find(|b| b.id == "47.4.10").unwrap();
    assert_eq!(rec.tag, Some(LoaderTag::Recommended));
    assert!(builds.iter().all(|b| !b.id.starts_with("1.20.1-")));

    // No mods: Forge's recommended build, as before.
    assert_eq!(choose(&builds, &[]).as_deref(), Some("47.4.10"));
    // TSP rules the recommended build out; the latest promotion is next.
    let dir = tempfile::tempdir().unwrap();
    write_jar(&dir.path().join("tsp.jar"), "META-INF/mods.toml", TSP_TOML);
    let reqs = requirements::scan(dir.path(), ServerType::Forge);
    assert_eq!(choose(&builds, &reqs).as_deref(), Some("47.4.26"));
    assert!(!accepts(&reqs, "47.4.10"));
}

#[test]
fn neoforge_and_fabric_list_builds_and_prefer_stable_ones() {
    let all = neoforge::parse_versions(&fixture("providers/neoforge_versions.json")).unwrap();
    let neo = neoforge::builds_for(&all, "1.21.1");
    assert!(neo.iter().all(|v| neoforge::mc_for(&v.id).as_deref() == Some("1.21.1")));
    assert_eq!(choose(&neo, &[]).as_deref(), Some("21.1.219"));

    let fab = fabric::parse_loaders(&fixture("providers/fabric_loader.json")).unwrap();
    assert_eq!(fab[0].id, "0.19.5");
    assert_eq!(fab[1].tag, Some(LoaderTag::Beta));
    assert_eq!(choose(&fab, &[]).as_deref(), Some("0.19.5"));
}

#[test]
fn installed_build_is_read_from_launch_files() {
    let forge_launch = LaunchInfo::ArgsFile { args_file: "libraries/net/minecraftforge/forge/1.20.1-47.4.10/win_args.txt".into() };
    assert_eq!(loader_from_launch(ServerType::Forge, "1.20.1", &forge_launch).as_deref(), Some("47.4.10"));
    let neo = LaunchInfo::ArgsFile { args_file: "libraries/net/neoforged/neoforge/21.1.219/win_args.txt".into() };
    assert_eq!(loader_from_launch(ServerType::Neoforge, "1.21.1", &neo).as_deref(), Some("21.1.219"));
    let fab = LaunchInfo::Jar { jar: r"C:\cache\fabric-26.3-loader0.19.5-launcher1.1.2.jar".into() };
    assert_eq!(loader_from_launch(ServerType::Fabric, "26.3", &fab).as_deref(), Some("0.19.5"));
    assert_eq!(loader_from_launch(ServerType::Paper, "26.3", &fab), None);
}

#[test]
fn reinstalling_forge_launches_the_new_build_not_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    for v in ["1.20.1-47.4.10", "1.20.1-47.4.26"] {
        let d = dir.path().join("libraries/net/minecraftforge/forge").join(v);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("win_args.txt"), "").unwrap();
    }
    for v in ["1.20.1-47.4.10", "1.20.1-47.4.26"] {
        match forge::detect_launch(dir.path(), Some(v)).unwrap() {
            LaunchInfo::ArgsFile { args_file } => assert!(args_file.contains(v), "{args_file} should be {v}"),
            other => panic!("unexpected {other:?}"),
        }
    }
}
