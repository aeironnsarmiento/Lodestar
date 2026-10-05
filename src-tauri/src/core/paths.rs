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

    /// `%APPDATA%\Glasscraft`.
    pub fn default_root() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir());
        base.join("Glasscraft")
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
