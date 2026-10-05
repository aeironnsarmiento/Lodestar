//! Instance and settings persistence. Every JSON write is atomic: the new content goes
//! to a temp file that is flushed and then renamed over the old one, so a crash
//! mid-write leaves the previous valid file in place.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::instance::{slugify, Difficulty, Instance, NewInstance, Provision};
use super::paths::Paths;
use super::settings::AppSettings;

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path);
    let json = serde_json::to_vec_pretty(value)?;
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("writing {}", tmp.display()))?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("{} is not valid", path.display()))
}

struct Inner {
    instances: BTreeMap<String, Instance>,
    settings: AppSettings,
}

pub struct Store {
    paths: Paths,
    inner: RwLock<Inner>,
    load_errors: Vec<String>,
}

impl Store {
    /// Loads settings and every instance folder. Unreadable instances are reported in
    /// [`Store::load_errors`] and skipped; the rest still load.
    pub fn open(paths: Paths) -> Result<Self> {
        fs::create_dir_all(paths.instances_dir())?;
        let mut load_errors = Vec::new();

        let settings_file = paths.settings_file();
        let settings = if settings_file.exists() {
            read_json(&settings_file).unwrap_or_else(|e| {
                load_errors.push(format!("{e:#}"));
                AppSettings::default()
            })
        } else {
            AppSettings::default()
        };

        let mut instances = BTreeMap::new();
        for entry in fs::read_dir(paths.instances_dir())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let file = entry.path().join("instance.json");
            if !file.exists() {
                continue;
            }
            match read_json::<Instance>(&file) {
                Ok(mut inst) => {
                    // The folder name is the identity; a hand-edited id cannot point elsewhere.
                    inst.id = entry.file_name().to_string_lossy().into_owned();
                    // Provisioning cannot survive an app restart; let the user retry it.
                    if let Provision::Running { .. } = inst.provision {
                        inst.provision = Provision::Failed {
                            message: "Setup was interrupted. Retry to finish it.".into(),
                        };
                    }
                    instances.insert(inst.id.clone(), inst);
                }
                Err(e) => load_errors.push(format!("{e:#}")),
            }
        }

        Ok(Self {
            paths,
            inner: RwLock::new(Inner { instances, settings }),
            load_errors,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }

    /// Instances in creation order.
    pub fn list(&self) -> Vec<Instance> {
        let inner = self.inner.read().unwrap();
        let mut list: Vec<Instance> = inner.instances.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        list
    }

    pub fn get(&self, id: &str) -> Result<Instance> {
        self.inner
            .read()
            .unwrap()
            .instances
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("No server with id '{id}'."))
    }

    /// Creates the record and its folders. Provisioning (server files, Java) is separate.
    pub fn create(&self, new: NewInstance) -> Result<Instance> {
        let name = new.name.trim();
        if name.is_empty() {
            bail!("Give the server a name.");
        }
        if new.mc_version.trim().is_empty() {
            bail!("Pick a Minecraft version.");
        }
        let mut inner = self.inner.write().unwrap();

        let base = slugify(name);
        let mut id = base.clone();
        let mut n = 2;
        while inner.instances.contains_key(&id) || self.paths.instance_dir(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }

        // Give each new server its own port so several can run at once (R9).
        let used: Vec<u16> = inner.instances.values().map(|i| i.port).collect();
        let mut port = 25565;
        while used.contains(&port) {
            port += 1;
        }

        let defaults = Instance::default();
        let inst = Instance {
            id: id.clone(),
            name: name.to_string(),
            server_type: new.server_type,
            mc_version: new.mc_version.trim().to_string(),
            initial_seed: new.seed.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            game_mode: new.game_mode,
            difficulty: if new.hardcore { Difficulty::Hard } else { new.difficulty },
            hardcore: new.hardcore,
            max_players: new.max_players.unwrap_or(defaults.max_players).clamp(1, 1000),
            port,
            created_at: chrono::Local::now().to_rfc3339(),
            ..defaults
        };
        fs::create_dir_all(self.paths.server_dir(&id))?;
        write_json_atomic(&self.paths.instance_file(&id), &inst)?;
        inner.instances.insert(id, inst.clone());
        Ok(inst)
    }

    /// Replaces a record. The id, folder and creation time cannot change.
    pub fn update(&self, mut inst: Instance) -> Result<Instance> {
        let mut inner = self.inner.write().unwrap();
        let existing = inner
            .instances
            .get(&inst.id)
            .ok_or_else(|| anyhow!("No server with id '{}'.", inst.id))?;
        inst.created_at = existing.created_at.clone();
        if inst.name.trim().is_empty() {
            bail!("Give the server a name.");
        }
        inst.normalize();
        write_json_atomic(&self.paths.instance_file(&inst.id), &inst)?;
        inner.instances.insert(inst.id.clone(), inst.clone());
        Ok(inst)
    }

    /// Read-modify-write under one lock.
    pub fn modify(&self, id: &str, f: impl FnOnce(&mut Instance)) -> Result<Instance> {
        let mut inner = self.inner.write().unwrap();
        let inst = inner
            .instances
            .get_mut(id)
            .ok_or_else(|| anyhow!("No server with id '{id}'."))?;
        f(inst);
        let inst = inst.clone();
        write_json_atomic(&self.paths.instance_file(id), &inst)?;
        Ok(inst)
    }

    /// Deletes the instance and all its files. Refused while the server is running.
    pub fn delete(&self, id: &str, running: bool) -> Result<()> {
        let mut inner = self.inner.write().unwrap();
        let inst = inner
            .instances
            .get(id)
            .ok_or_else(|| anyhow!("No server with id '{id}'."))?;
        if running {
            bail!("Stop \"{}\" before deleting it.", inst.name);
        }
        let dir = self.paths.instance_dir(id);
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("deleting {}", dir.display()))?;
        }
        inner.instances.remove(id);
        Ok(())
    }

    pub fn settings(&self) -> AppSettings {
        self.inner.read().unwrap().settings.clone()
    }

    pub fn set_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let mut inner = self.inner.write().unwrap();
        write_json_atomic(&self.paths.settings_file(), &settings)?;
        inner.settings = settings.clone();
        Ok(settings)
    }

    pub fn modify_settings(&self, f: impl FnOnce(&mut AppSettings)) -> Result<AppSettings> {
        let mut next = self.settings();
        f(&mut next);
        self.set_settings(next)
    }
}
