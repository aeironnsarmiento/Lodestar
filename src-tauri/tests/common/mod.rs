//! Test helpers: a tiny local HTTP server so no test touches the real network, and
//! fixture loading.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self { status: 200, body: body.into() }
    }

    pub fn status(status: u16) -> Self {
        Self { status, body: Vec::new() }
    }

    pub fn json(v: serde_json::Value) -> Self {
        Self::ok(v.to_string())
    }
}

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

pub struct TestServer {
    pub base: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl TestServer {
    pub fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);
        let log = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let handler = handler.clone();
                let log = log.clone();
                thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_string();
                    let path = parts.next().unwrap_or("").to_string();
                    let mut headers = Vec::new();
                    loop {
                        let mut h = String::new();
                        if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                            break;
                        }
                        if let Some((k, v)) = h.trim_end().split_once(':') {
                            headers.push((k.trim().to_string(), v.trim().to_string()));
                        }
                    }
                    let len = headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, v)| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    let mut body = vec![0; len];
                    reader.read_exact(&mut body).ok();
                    let req = Request { method, path, headers, body };
                    log.lock().unwrap().push(req.clone());
                    let resp = handler(&req);
                    let head = format!(
                        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                        resp.status,
                        resp.body.len()
                    );
                    stream.write_all(head.as_bytes()).ok();
                    stream.write_all(&resp.body).ok();
                    stream.flush().ok();
                });
            }
        });
        Self { base, requests }
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    pub fn count(&self, path_prefix: &str) -> usize {
        self.requests().iter().filter(|r| r.path.starts_with(path_prefix)).count()
    }
}

pub fn fixture_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(rel)
}

pub fn fixture(rel: &str) -> String {
    std::fs::read_to_string(fixture_path(rel)).unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

pub fn sha1_hex(data: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(data))
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(data))
}

/// Serves the recorded fixtures with every download URL rewritten to the test server.
pub fn fixture_server() -> TestServer {
    TestServer::start(|req| {
        let base = format!("http://{}", req.header("host").unwrap());
        let p = req.path.as_str();
        match p {
            "/mojang/version_manifest_v2.json" => {
                let mut m: serde_json::Value = serde_json::from_str(&fixture("providers/mojang_manifest.json")).unwrap();
                for v in m["versions"].as_array_mut().unwrap() {
                    let id = v["id"].as_str().unwrap().to_string();
                    v["url"] = format!("{base}/mojang/v/{id}.json").into();
                }
                Response::json(m)
            }
            "/mojang/v/26.3.json" | "/mojang/v/1.16.1.json" => {
                let name = p.trim_start_matches("/mojang/v/");
                let mut d: serde_json::Value = serde_json::from_str(&fixture(&format!("providers/mojang_{name}"))).unwrap();
                d["downloads"]["server"]["url"] = format!("{base}/files/vanilla-server.jar").into();
                d["downloads"]["server"]["sha1"] = sha1_hex(b"vanilla server").into();
                Response::json(d)
            }
            "/files/vanilla-server.jar" => Response::ok(b"vanilla server".to_vec()),
            "/paper/versions/26.3/builds/latest" => {
                let mut b: serde_json::Value = serde_json::from_str(&fixture("providers/paper_build_latest.json")).unwrap();
                b["downloads"]["server:default"]["url"] = format!("{base}/files/paper.jar").into();
                b["downloads"]["server:default"]["checksums"]["sha256"] = sha256_hex(b"paper server").into();
                Response::json(b)
            }
            "/files/paper.jar" => Response::ok(b"paper server".to_vec()),
            "/fabric/versions/game" => Response::ok(fixture("providers/fabric_game.json")),
            "/fabric/versions/loader" => Response::ok(fixture("providers/fabric_loader.json")),
            "/fabric/versions/installer" => Response::ok(fixture("providers/fabric_installer.json")),
            "/fabric/versions/loader/26.3/0.19.5/1.1.2/server/jar" => Response::ok(b"fabric launcher".to_vec()),
            "/paper" => Response::ok(fixture("providers/paper_project.json")),
            "/forge/maven/maven-metadata.xml" => Response::ok(fixture("providers/forge_maven_metadata.xml")),
            "/neoforge/api" => Response::ok(fixture("providers/neoforge_versions.json")),
            _ if p.starts_with("/modrinth/project/lithium/") => {
                let mut l: serde_json::Value = serde_json::from_str(&fixture("providers/modrinth_lithium_26.3.json")).unwrap();
                for v in l.as_array_mut().unwrap() {
                    for f in v["files"].as_array_mut().unwrap() {
                        f["url"] = format!("{base}/files/lithium.jar").into();
                        f["hashes"]["sha1"] = sha1_hex(b"lithium").into();
                    }
                }
                Response::json(l)
            }
            "/files/lithium.jar" => Response::ok(b"lithium".to_vec()),
            _ if p.starts_with("/modrinth/project/ferrite-core/") => Response::ok(fixture("providers/modrinth_empty.json")),
            _ if p.starts_with("/adoptium/assets/latest/") => {
                let major: u32 = p["/adoptium/assets/latest/".len()..].split('/').next().unwrap().parse().unwrap();
                let zip = jre_zip(major);
                Response::json(serde_json::json!([{ "binary": { "package": {
                    "link": format!("{base}/files/jre-{major}.zip"),
                    "checksum": sha256_hex(&zip),
                    "name": format!("OpenJDK{major}U-jre_x64_windows_hotspot.zip"),
                }}}]))
            }
            _ if p.starts_with("/files/jre-") => {
                let major: u32 = p.trim_start_matches("/files/jre-").trim_end_matches(".zip").parse().unwrap();
                Response::ok(jre_zip(major))
            }
            _ => Response::status(404),
        }
    })
}

/// A fake Temurin JRE zip: one top-level folder with `bin/java.exe` and a `release` file.
pub fn jre_zip(major: u32) -> Vec<u8> {
    use std::io::Write as _;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        let top = format!("jdk-{major}.0.1+9-jre");
        z.start_file(format!("{top}/bin/java.exe"), opts).unwrap();
        z.write_all(b"not a real java").unwrap();
        z.start_file(format!("{top}/release"), opts).unwrap();
        z.write_all(format!("JAVA_VERSION=\"{major}.0.1\"
").as_bytes()).unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// An `App` rooted at `root` whose servers run `fake_mc` instead of Java and whose
/// downloads go to `server`.
pub fn test_app(server: &TestServer, root: &std::path::Path) -> std::sync::Arc<glasscraft_lib::core::app::App> {
    test_app_with(server, root, |_| {})
}

pub fn test_app_with(
    server: &TestServer,
    root: &std::path::Path,
    tweak: impl FnOnce(&mut glasscraft_lib::core::app::AppConfig),
) -> std::sync::Arc<glasscraft_lib::core::app::App> {
    use glasscraft_lib::core::app::{App, AppConfig};
    let mut config = AppConfig::new(glasscraft_lib::core::paths::Paths::new(root));
    config.endpoints = glasscraft_lib::providers::Endpoints::local(&server.base);
    config.adoptium_api = format!("{}/adoptium", server.base);
    config.download_backoff = std::time::Duration::from_millis(5);
    config.java_override = Some(PathBuf::from(env!("CARGO_BIN_EXE_fake_mc")));
    tweak(&mut config);
    App::new(config, glasscraft_lib::core::events::MemorySink::new()).unwrap()
}

/// Creates an instance that is already provisioned (no downloads), on a free port,
/// with the EULA accepted.
pub fn ready_instance(app: &glasscraft_lib::core::app::App, name: &str) -> glasscraft_lib::core::instance::Instance {
    use glasscraft_lib::core::instance::{LaunchInfo, NewInstance, Provision, ServerType};
    app.accept_eula().unwrap();
    let inst = app
        .create_instance(NewInstance {
            name: name.into(),
            server_type: ServerType::Vanilla,
            mc_version: "26.3".into(),
            ..Default::default()
        })
        .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    app.store
        .modify(&inst.id, |i| {
            i.provision = Provision::Ready;
            i.launch = Some(LaunchInfo::Jar { jar: "server.jar".into() });
            i.java_major = Some(25);
            i.port = port;
        })
        .unwrap()
}

/// Console text of an instance.
pub fn console_text(app: &glasscraft_lib::core::app::App, id: &str) -> Vec<String> {
    app.console(id).into_iter().map(|l| l.text).collect()
}
