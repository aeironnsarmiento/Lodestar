mod common;

use std::time::Duration;

use common::{fixture_server, sha256_hex, Response, TestServer};
use lodestar_lib::core::paths::Paths;
use lodestar_lib::download::{no_progress, Downloader};
use lodestar_lib::java::{parse_java_version, target_major, JavaManager};
use lodestar_lib::providers::{Endpoints, Providers};

fn manager(server: &TestServer, root: &std::path::Path) -> JavaManager {
    JavaManager::new(
        Paths::new(root).java_runtimes_dir(),
        Downloader::new().with_backoff(Duration::from_millis(5)),
        format!("{}/adoptium", server.base),
    )
}

#[test]
fn requirements_map_to_temurin_lts_runtimes() {
    assert_eq!(target_major(25), 25);
    assert_eq!(target_major(21), 21);
    assert_eq!(target_major(17), 17);
    assert_eq!(target_major(16), 17);
    assert_eq!(target_major(8), 8);
    assert_eq!(target_major(7), 8);
    assert_eq!(target_major(22), 25);
    assert_eq!(target_major(26), 26, "a non-LTS requirement above 25 maps to itself");
}

#[test]
fn java_version_output_parses_old_and_new_formats() {
    let old = "openjdk version \"1.8.0_412\"\nOpenJDK Runtime Environment (Temurin)(build 1.8.0_412-b08)";
    assert_eq!(parse_java_version(old), Some(8));
    let new = "openjdk version \"25.0.1\" 2025-10-21 LTS\nOpenJDK Runtime Environment Temurin-25.0.1+8";
    assert_eq!(parse_java_version(new), Some(25));
    assert_eq!(parse_java_version("java version \"21\" 2023-09-19"), Some(21));
    assert_eq!(parse_java_version("garbage"), None);
}

#[tokio::test]
async fn installs_once_and_reuses_an_installed_runtime() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let java = manager(&server, dir.path());

    let exe = java.ensure(21, &no_progress).await.unwrap();
    assert_eq!(exe, Paths::new(dir.path()).java_dir(21).join("bin").join("java.exe"));
    assert!(exe.is_file());
    assert!(!Paths::new(dir.path()).java_runtimes_dir().join("jre-21.zip").exists(), "zip is cleaned up");
    let downloads = server.count("/files/jre-21.zip");
    assert_eq!(downloads, 1);

    // Already installed: no network at all.
    let before = server.requests().len();
    assert_eq!(java.ensure(21, &no_progress).await.unwrap(), exe);
    assert_eq!(server.requests().len(), before);

    let installed = java.installed();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].major, 21);
    assert!(installed[0].size_bytes > 0);
}

#[tokio::test]
async fn a_runtime_folder_without_java_exe_is_repaired() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let java = manager(&server, dir.path());
    let broken = Paths::new(dir.path()).java_dir(17);
    std::fs::create_dir_all(broken.join("lib")).unwrap();
    std::fs::write(broken.join("lib").join("leftover"), b"x").unwrap();

    assert!(java.installed().is_empty(), "a folder without java.exe is not installed");
    let exe = java.ensure(16, &no_progress).await.unwrap();
    assert!(exe.ends_with("jre-17/bin/java.exe") || exe.ends_with(r"jre-17\bin\java.exe"));
    assert!(exe.is_file());
    assert!(!broken.join("lib").join("leftover").exists(), "the broken folder was replaced");
}

#[tokio::test]
async fn an_extraction_failure_cleans_up_and_reports() {
    let bad_zip = b"this is not a zip archive".to_vec();
    let checksum = sha256_hex(&bad_zip);
    let server = TestServer::start(move |req| {
        let base = format!("http://{}", req.header("host").unwrap());
        if req.path.starts_with("/adoptium/") {
            Response::json(serde_json::json!([{ "binary": { "package": {
                "link": format!("{base}/files/jre-25.zip"), "checksum": checksum, "name": "jre.zip" }}}]))
        } else {
            Response::ok(bad_zip.clone())
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let java = manager(&server, dir.path());
    let err = java.ensure(25, &no_progress).await.unwrap_err();
    assert!(format!("{err:#}").contains("Java 25"), "{err:#}");

    let runtimes = Paths::new(dir.path()).java_runtimes_dir();
    let leftovers: Vec<String> = std::fs::read_dir(&runtimes)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}

#[tokio::test]
async fn removing_a_runtime_in_use_is_refused() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let java = manager(&server, dir.path());
    java.ensure(25, &no_progress).await.unwrap();
    assert!(java.remove(25, true).unwrap_err().to_string().contains("in use"));
    java.remove(25, false).unwrap();
    assert!(java.installed().is_empty());
}

#[tokio::test]
async fn instances_for_1_16_1_and_26_3_resolve_java_8_and_25_without_the_system_java() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let providers = Providers::new(
        Paths::new(dir.path()),
        Downloader::new().with_backoff(Duration::from_millis(5)),
        Endpoints::local(&server.base),
    );
    let java = manager(&server, dir.path());
    let runtimes = Paths::new(dir.path()).java_runtimes_dir();

    for (mc, want) in [("1.16.1", 8), ("26.3", 25)] {
        let required = providers.java_requirement(mc).await.unwrap();
        let exe = java.ensure(required, &no_progress).await.unwrap();
        assert!(exe.starts_with(&runtimes), "{mc} must use a managed runtime, got {}", exe.display());
        assert!(exe.to_string_lossy().contains(&format!("jre-{want}")), "{mc} → {}", exe.display());
    }
}

/// Opt-in: `LODESTAR_LIVE_TESTS=1 cargo test live_`. Asks Adoptium for the Windows
/// JREs the app maps to, without downloading them.
#[tokio::test]
async fn live_adoptium_has_windows_jres_for_every_target() {
    if std::env::var("LODESTAR_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipped: set LODESTAR_LIVE_TESTS=1 to run");
        return;
    }
    let d = Downloader::new();
    for major in [8, 17, 21, 25] {
        let url = format!(
            "{}/assets/latest/{major}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse",
            JavaManager::DEFAULT_API
        );
        let assets: Vec<serde_json::Value> = d.get_json(&url).await.unwrap();
        let pkg = &assets[0]["binary"]["package"];
        assert!(pkg["name"].as_str().unwrap().ends_with(".zip"), "Java {major}");
        assert_eq!(pkg["checksum"].as_str().unwrap().len(), 64, "Java {major}");
    }
}
