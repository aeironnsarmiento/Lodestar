mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{sha1_hex, sha256_hex, Response, TestServer};
use lodestar_lib::download::{extract_zip_flatten, no_progress, Downloader, Hash};

fn fast() -> Downloader {
    Downloader::new().with_backoff(Duration::from_millis(10))
}

#[tokio::test]
async fn wrong_hash_deletes_the_file_and_reports_it() {
    let server = TestServer::start(|_| Response::ok(b"not the real jar".to_vec()));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("cache").join("server.jar");

    let err = fast()
        .download(&format!("{}/server.jar", server.base), &dest, Some(&Hash::Sha1("0".repeat(40))), &no_progress)
        .await
        .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("SHA-1"), "{msg}");
    assert!(!dest.exists(), "a corrupt file must not be promoted into the cache");
    assert!(!dir.path().join("cache").join("server.jar.part").exists());
    // A hash mismatch is not retried.
    assert_eq!(server.count("/server.jar"), 1);
}

#[tokio::test]
async fn transient_failures_are_retried_until_the_file_completes() {
    let body = b"jar bytes".to_vec();
    let hash = sha256_hex(&body);
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let server = TestServer::start(move |_| {
        if h.fetch_add(1, Ordering::SeqCst) < 2 {
            Response::status(503)
        } else {
            Response::ok(body.clone())
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("paper.jar");
    let progress_calls = Arc::new(AtomicUsize::new(0));
    let p = progress_calls.clone();
    let progress = move |_: u64, _: Option<u64>| {
        p.fetch_add(1, Ordering::SeqCst);
    };

    fast()
        .download(&format!("{}/paper.jar", server.base), &dest, Some(&Hash::Sha256(hash)), &progress)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"jar bytes");
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert!(progress_calls.load(Ordering::SeqCst) > 0);
}

#[tokio::test]
async fn gives_up_after_three_attempts_and_leaves_nothing_behind() {
    let server = TestServer::start(|_| Response::status(500));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("x.jar");
    let err = fast().download(&format!("{}/x.jar", server.base), &dest, None, &no_progress).await.unwrap_err();
    assert!(format!("{err:#}").contains("after 3 attempts"));
    assert_eq!(server.count("/x.jar"), 3);
    assert!(!dest.exists());
    assert!(!dir.path().join("x.jar.part").exists());
}

#[tokio::test]
async fn not_found_is_not_retried() {
    let server = TestServer::start(|_| Response::status(404));
    let dir = tempfile::tempdir().unwrap();
    let err = fast()
        .download(&format!("{}/missing.jar", server.base), &dir.path().join("m.jar"), None, &no_progress)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("404"));
    assert_eq!(server.count("/missing.jar"), 1);
}

#[tokio::test]
async fn cached_file_with_matching_hash_is_reused_without_network() {
    let server = TestServer::start(|_| Response::ok(b"fresh".to_vec()));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("vanilla-26.3.jar");
    std::fs::write(&dest, b"cached jar").unwrap();
    let hash = Hash::Sha1(sha1_hex(b"cached jar"));

    fast().download(&format!("{}/v.jar", server.base), &dest, Some(&hash), &no_progress).await.unwrap();
    assert_eq!(server.requests().len(), 0);
    assert_eq!(std::fs::read(&dest).unwrap(), b"cached jar");

    // A cached file whose hash no longer matches is replaced.
    let good = Hash::Sha1(sha1_hex(b"fresh"));
    fast().download(&format!("{}/v.jar", server.base), &dest, Some(&good), &no_progress).await.unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"fresh");
}

#[test]
fn zip_with_one_top_folder_is_flattened() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("jre.zip");
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts: zip::write::SimpleFileOptions = Default::default();
        z.add_directory("jdk-25.0.4+1-jre/bin/", opts).unwrap();
        z.start_file("jdk-25.0.4+1-jre/bin/java.exe", opts).unwrap();
        z.write_all(b"MZ").unwrap();
        z.start_file("jdk-25.0.4+1-jre/release", opts).unwrap();
        z.write_all(b"JAVA_VERSION=\"25.0.4\"").unwrap();
        z.finish().unwrap();
    }
    let dest = dir.path().join("runtimes").join("jre-25");
    extract_zip_flatten(&zip_path, &dest).unwrap();
    assert!(dest.join("bin").join("java.exe").is_file());
    assert!(dest.join("release").is_file());
}

#[test]
fn a_broken_zip_cleans_up_its_temp_folder() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("bad.zip");
    std::fs::write(&zip_path, b"definitely not a zip").unwrap();
    let dest = dir.path().join("runtimes").join("jre-21");
    assert!(extract_zip_flatten(&zip_path, &dest).is_err());
    assert!(!dest.exists());
    let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("runtimes")).unwrap().collect();
    assert!(leftovers.is_empty(), "temp extraction folder was left behind");
}
