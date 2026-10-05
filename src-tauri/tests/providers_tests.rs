mod common;

use std::path::Path;
use std::time::Duration;

use common::{fixture, fixture_server, TestServer};
use lodestar_lib::core::instance::{LaunchInfo, ServerType};
use lodestar_lib::core::paths::Paths;
use lodestar_lib::download::{no_progress, Downloader};
use lodestar_lib::providers::modrinth::{self, ModOutcome};
use lodestar_lib::providers::{
    compare_versions, fabric, forge, launch_spec, mojang, neoforge, paper, Endpoints, Providers, VersionKind,
};

#[test]
fn mojang_manifest_lists_releases_newest_first_with_dates_and_tags_snapshots() {
    let m = mojang::parse_manifest(&fixture("providers/mojang_manifest.json")).unwrap();
    assert_eq!(m.latest.release, "26.3");
    let all = mojang::versions(&m);

    let releases: Vec<&str> = all.iter().filter(|v| v.kind == VersionKind::Release).map(|v| v.id.as_str()).collect();
    assert_eq!(&releases[..4], &["26.3", "26.2", "26.1.2", "1.21.11"]);
    assert!(all.iter().all(|v| v.release_time.is_some()));
    // Old alpha/beta versions are not offered.
    assert!(!all.iter().any(|v| v.id == "b1.7.3"));

    // The snapshot filter adds snapshots, still newest first.
    assert_eq!(all[0].id, "26.4-snapshot-2");
    assert_eq!(all[0].kind, VersionKind::Snapshot);
    assert!(all.iter().any(|v| v.id == "26.3-rc-3" && v.kind == VersionKind::Snapshot));
}

#[test]
fn mojang_version_details_give_java_requirement_and_server_jar() {
    let d = mojang::parse_version_details(&fixture("providers/mojang_26.3.json")).unwrap();
    assert_eq!(d.java_major, 25);
    let server = d.server.unwrap();
    assert!(server.url.ends_with("server.jar"));
    assert_eq!(server.sha1.len(), 40);

    let old = mojang::parse_version_details(&fixture("providers/mojang_1.16.1.json")).unwrap();
    assert_eq!(old.java_major, 8);
    // Versions that predate `javaVersion` default to Java 8.
    assert_eq!(mojang::parse_version_details("{}").unwrap().java_major, 8);
}

#[test]
fn paper_build_yields_server_default_url_and_sha256() {
    let b = paper::parse_build(&fixture("providers/paper_build_latest.json")).unwrap();
    assert_eq!(b.name, "paper-26.3-152.jar");
    assert!(b.url.starts_with("https://fill-data.papermc.io/"));
    assert_eq!(b.sha256, "034b99f3278d985f5d723ad9b02c65c459600223fa489534ebb4e9bbbb3f68e9");

    let versions = paper::parse_project(&fixture("providers/paper_project.json")).unwrap();
    assert_eq!(versions[0].id, "26.3");
    assert!(versions.iter().any(|v| v.id == "26.3-rc-3" && v.kind == VersionKind::Snapshot));
    assert!(versions.iter().any(|v| v.id == "1.21.11-rc3" && v.kind == VersionKind::Snapshot));
    let pos = |id: &str| versions.iter().position(|v| v.id == id).unwrap();
    assert!(pos("1.21.11") < pos("1.21.10") && pos("1.21.10") < pos("1.21.9") && pos("1.21.9") < pos("1.20.6"));
}

#[test]
fn fabric_meta_parses_game_versions_and_stable_components() {
    let games = fabric::parse_game_versions(&fixture("providers/fabric_game.json")).unwrap();
    assert_eq!(games[0].kind, VersionKind::Snapshot);
    assert!(games.iter().any(|g| g.id == "26.3" && g.kind == VersionKind::Release));
    assert_eq!(fabric::pick_stable(&fixture("providers/fabric_loader.json")).unwrap(), "0.19.5");
    assert_eq!(fabric::pick_stable(&fixture("providers/fabric_installer.json")).unwrap(), "1.1.2");
}

#[test]
fn forge_fixtures_yield_26_3_builds_and_exclude_versions_below_1_17() {
    let meta = forge::parse_maven_metadata(&fixture("providers/forge_maven_metadata.xml"));
    let promos = forge::parse_promotions(&fixture("providers/forge_promotions_slim.json")).unwrap();
    let mcs: Vec<String> = forge::mc_versions(&meta).into_iter().map(|v| v.id).collect();

    assert_eq!(mcs[0], "26.3");
    assert!(mcs.contains(&"1.17.1".to_string()));
    assert!(mcs.contains(&"1.20.1".to_string()));
    for old in ["1.16.5", "1.12.2", "1.7.10"] {
        assert!(!mcs.iter().any(|m| m == old), "{old} must be excluded");
    }
    assert!(meta.iter().all(|(mc, _)| !mc.contains('_')));

    // 26.3 has a "latest" promotion; the build matches it.
    let v = forge::pick_version(&meta, &promos, "26.3").unwrap();
    assert_eq!(v, format!("26.3-{}", promos["26.3-latest"]));
    // Recommended wins over latest when both exist.
    assert_eq!(forge::pick_version(&meta, &promos, "26.2").unwrap(), format!("26.2-{}", promos["26.2-recommended"]));
    // Without promotions, the numerically newest build is used (not the XML order).
    let none = Default::default();
    assert_eq!(forge::pick_version(&meta, &none, "1.21.1").unwrap(), "1.21.1-52.1.16");
    assert_eq!(forge::pick_version(&meta, &none, "1.16.5"), None);
    assert_eq!(
        forge::installer_url("https://maven.minecraftforge.net/net/minecraftforge/forge", &v),
        format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{v}/forge-{v}-installer.jar")
    );
}

#[test]
fn neoforge_versions_map_to_minecraft_versions() {
    assert_eq!(neoforge::mc_for("26.3.0.48-beta").as_deref(), Some("26.3"));
    assert_eq!(neoforge::mc_for("26.3.1.4").as_deref(), Some("26.3.1"));
    assert_eq!(neoforge::mc_for("21.1.219").as_deref(), Some("1.21.1"));
    assert_eq!(neoforge::mc_for("21.11.5-beta").as_deref(), Some("1.21.11"));
    assert_eq!(neoforge::mc_for("20.2.3-beta").as_deref(), Some("1.20.2"));
    assert_eq!(neoforge::mc_for("0.25w14craftmine.3-beta"), None);

    let all = neoforge::parse_versions(&fixture("providers/neoforge_versions.json")).unwrap();
    let mcs: Vec<String> = neoforge::mc_versions(&all).into_iter().map(|v| v.id).collect();
    assert_eq!(mcs[0], "26.3");
    assert!(mcs.contains(&"1.21.1".to_string()));
    assert_eq!(neoforge::pick_version(&all, "26.3").as_deref(), Some("26.3.0.48-beta"));
    // Stable builds are preferred over betas.
    assert_eq!(neoforge::pick_version(&all, "1.21.1").as_deref(), Some("21.1.219"));
}

#[test]
fn forge_install_directory_launches_with_args_files_not_run_bat() {
    let dir = tempfile::tempdir().unwrap();
    let server = dir.path();
    let args_dir = server.join("libraries/net/minecraftforge/forge/26.3-66.0.9");
    std::fs::create_dir_all(&args_dir).unwrap();
    std::fs::write(args_dir.join("win_args.txt"), "-p libraries/... net.minecraftforge.bootstrap.ForgeBootstrap").unwrap();
    std::fs::write(server.join("user_jvm_args.txt"), "# custom JVM args").unwrap();
    std::fs::write(server.join("run.bat"), "java @user_jvm_args.txt @libraries/... %*").unwrap();

    let launch = forge::detect_launch(server).unwrap();
    assert_eq!(
        launch,
        LaunchInfo::ArgsFile { args_file: "libraries/net/minecraftforge/forge/26.3-66.0.9/win_args.txt".into() }
    );

    let java = Path::new(r"C:\runtimes\jre-25\bin\java.exe");
    let spec = launch_spec(java, server, &launch, 4096, &["--universe".into(), "worlds/run_1".into()]);
    assert_eq!(spec.program, java);
    assert_eq!(spec.working_dir, server);
    assert!(spec.args.contains(&"@user_jvm_args.txt".to_string()));
    assert!(spec.args.contains(&"@libraries/net/minecraftforge/forge/26.3-66.0.9/win_args.txt".to_string()));
    assert!(spec.args.contains(&"-Xmx4096M".to_string()));
    assert!(!spec.args.iter().any(|a| a.contains("run.bat") || a == "-jar"));
    let tail: Vec<&str> = spec.args.iter().rev().take(3).map(String::as_str).collect();
    assert_eq!(tail, vec!["worlds/run_1", "--universe", "nogui"]);
}

#[test]
fn installs_without_launch_files_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    assert!(forge::detect_launch(dir.path()).is_err());
    std::fs::write(dir.path().join("forge-26.3-66.0.9-shim.jar"), b"PK").unwrap();
    match forge::detect_launch(dir.path()).unwrap() {
        LaunchInfo::Jar { jar } => assert!(jar.ends_with("forge-26.3-66.0.9-shim.jar")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn version_ordering_handles_year_versions_and_prereleases() {
    use std::cmp::Ordering::*;
    assert_eq!(compare_versions("26.3", "1.21.11"), Greater);
    assert_eq!(compare_versions("1.21.11", "1.21.9"), Greater);
    assert_eq!(compare_versions("1.21.11-rc3", "1.21.11"), Less);
    assert_eq!(compare_versions("26.3.1", "26.3"), Greater);
}

#[test]
fn modrinth_pick_uses_the_primary_file_and_none_when_no_build_exists() {
    let pick = modrinth::pick(&fixture("providers/modrinth_lithium_26.3.json")).unwrap().unwrap();
    assert!(pick.filename.ends_with(".jar"));
    assert_eq!(pick.sha1.len(), 40);
    assert_eq!(modrinth::pick("[]").unwrap(), None);
}

fn providers(server: &TestServer, root: &Path) -> Providers {
    Providers::new(
        Paths::new(root),
        Downloader::new().with_backoff(Duration::from_millis(5)),
        Endpoints::local(&server.base),
    )
}

#[tokio::test]
async fn vanilla_paper_and_fabric_install_to_the_shared_jar_cache() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let p = providers(&server, dir.path());
    let server_dir = dir.path().join("instances/a/server");
    let cache = Paths::new(dir.path()).jar_cache_dir();

    let vanilla = p.install(ServerType::Vanilla, "26.3", &server_dir, None, &no_progress).await.unwrap();
    assert_eq!(vanilla, LaunchInfo::Jar { jar: cache.join("vanilla-26.3.jar").to_string_lossy().into() });
    let paper = p.install(ServerType::Paper, "26.3", &server_dir, None, &no_progress).await.unwrap();
    assert_eq!(paper, LaunchInfo::Jar { jar: cache.join("paper-26.3-152.jar").to_string_lossy().into() });
    let fabric = p.install(ServerType::Fabric, "26.3", &server_dir, None, &no_progress).await.unwrap();
    let LaunchInfo::Jar { jar } = &fabric else { panic!() };
    assert!(jar.ends_with("fabric-26.3-loader0.19.5-launcher1.1.2.jar"));
    assert_eq!(std::fs::read(jar).unwrap(), b"fabric launcher");

    // Each produces a valid launch spec.
    let spec = launch_spec(Path::new("java.exe"), &server_dir, &paper, 2048, &[]);
    let i = spec.args.iter().position(|a| a == "-jar").unwrap();
    assert!(spec.args[i + 1].ends_with("paper-26.3-152.jar"));
    assert_eq!(spec.args.last().unwrap(), "nogui");

    // A second install reuses the cached jar (no second download request).
    let before = server.count("/files/vanilla-server.jar");
    p.install(ServerType::Vanilla, "26.3", &server_dir, None, &no_progress).await.unwrap();
    assert_eq!(server.count("/files/vanilla-server.jar"), before);
}

#[tokio::test]
async fn version_lists_and_java_requirements_come_from_providers() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let p = providers(&server, dir.path());

    assert_eq!(p.java_requirement("26.3").await.unwrap(), 25);
    assert_eq!(p.java_requirement("1.16.1").await.unwrap(), 8);

    let paper = p.versions(ServerType::Paper).await.unwrap();
    // Release dates come from Mojang's manifest when the version exists there.
    assert!(paper.iter().find(|v| v.id == "26.3").unwrap().release_time.is_some());
    let forge = p.versions(ServerType::Forge).await.unwrap();
    assert_eq!(forge[0].id, "26.3");
    let neo = p.versions(ServerType::Neoforge).await.unwrap();
    assert_eq!(neo[0].id, "26.3");

    // Lists are cached for the session.
    let manifest_hits = server.count("/mojang/version_manifest_v2.json");
    p.versions(ServerType::Paper).await.unwrap();
    assert_eq!(server.count("/paper"), 1);
    assert_eq!(server.count("/mojang/version_manifest_v2.json"), manifest_hits);
}

#[tokio::test]
async fn modded_install_rejects_old_minecraft_and_needs_java() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let p = providers(&server, dir.path());
    let err = p.install(ServerType::Forge, "1.16.5", dir.path(), None, &no_progress).await.unwrap_err();
    assert!(err.to_string().contains("1.17"));
    let err = p.install(ServerType::Neoforge, "26.3", dir.path(), None, &no_progress).await.unwrap_err();
    assert!(err.to_string().contains("Java"));
}

#[tokio::test]
async fn speed_mods_install_and_a_missing_build_is_skipped() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("lithium-old.jar"), b"old").unwrap();
    std::fs::write(mods.join("my-own-mod.jar"), b"mine").unwrap();

    let out = modrinth::install_speed_mods(
        &Downloader::new().with_backoff(Duration::from_millis(5)),
        &format!("{}/modrinth", server.base),
        "26.3",
        &mods,
        &["lithium-old.jar".to_string()],
        &no_progress,
    )
    .await
    .unwrap();

    assert!(matches!(&out[0], ModOutcome::Installed { slug, .. } if slug == "lithium"));
    assert!(matches!(&out[1], ModOutcome::Skipped { slug, reason } if slug == "ferrite-core" && reason.contains("no build")));
    assert!(!mods.join("lithium-old.jar").exists(), "old managed mods are removed");
    assert!(mods.join("my-own-mod.jar").exists(), "user mods are kept");
}

/// Opt-in check against the real services: `LODESTAR_LIVE_TESTS=1 cargo test live_`.
/// Fetches version lists and metadata only; never downloads a server jar.
#[tokio::test]
async fn live_providers_resolve_current_versions() {
    if std::env::var("LODESTAR_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipped: set LODESTAR_LIVE_TESTS=1 to run");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let p = Providers::new(Paths::new(dir.path()), Downloader::new(), Endpoints::default());
    for t in [ServerType::Vanilla, ServerType::Paper, ServerType::Fabric, ServerType::Forge, ServerType::Neoforge] {
        let list = p.versions(t).await.unwrap();
        assert!(!list.is_empty(), "{t:?} returned no versions");
    }
    let manifest = p.manifest().await.unwrap();
    let java = p.java_requirement(&manifest.latest.release).await.unwrap();
    assert!(java >= 21);
    let d = Downloader::new();
    let build = d
        .get_text(&format!("{}/versions/{}/builds/latest", Endpoints::default().paper, manifest.latest.release))
        .await;
    if let Ok(text) = build {
        paper::parse_build(&text).unwrap();
    }
    let lithium = d
        .get_text(&modrinth::versions_url(&Endpoints::default().modrinth, "lithium", &manifest.latest.release))
        .await
        .unwrap();
    modrinth::pick(&lithium).unwrap();
}
