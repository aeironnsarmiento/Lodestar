//! Where everything lives on disk (KTD6). All paths derive from one root so tests
//! can point the whole app at a temporary folder.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `%APPDATA%\Lodestar`.
    pub fn default_root() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("Lodestar")
    }

    /// The app was called Glasscraft before. If `root` does not exist yet but the old
    /// `Glasscraft` folder beside it does, move it over so servers, worlds, Java and the
    /// playit.gg link carry across, and rename each server's console history file.
    pub fn migrate_legacy_root(root: &Path) -> std::io::Result<bool> {
        let Some(legacy) = root.parent().map(|p| p.join("Glasscraft")) else {
            return Ok(false);
        };
        if root.exists() || !legacy.is_dir() {
            return Ok(false);
        }
        std::fs::rename(&legacy, root)?;
        if let Ok(entries) = std::fs::read_dir(root.join("instances")) {
            for entry in entries.flatten() {
                let logs = entry.path().join("server").join("logs");
                let old = logs.join("glasscraft-console.log");
                if old.is_file() {
                    let _ = std::fs::rename(&old, logs.join("lodestar-console.log"));
                }
            }
        }
        Ok(true)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(id)
    }

    pub fn instance_file(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }

    pub fn server_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("server")
    }

    pub fn java_runtimes_dir(&self) -> PathBuf {
        self.root.join("runtimes").join("java")
    }

    pub fn java_dir(&self, major: u32) -> PathBuf {
        self.java_runtimes_dir().join(format!("jre-{major}"))
    }

    pub fn jar_cache_dir(&self) -> PathBuf {
        self.root.join("cache").join("jars")
    }

    pub fn playit_dir(&self) -> PathBuf {
        self.root.join("playit")
    }
}
