//! The backend service that Tauri commands call into. It owns the store and the
//! event sink; later units hang the supervisor, Java, worlds and playit off it.

use std::sync::Arc;

use anyhow::Result;

use super::events::{self, Events};
use super::instance::{Instance, NewInstance};
use super::paths::Paths;
use super::settings::AppSettings;
use super::store::Store;

pub struct App {
    pub store: Arc<Store>,
    pub events: Events,
}

impl App {
    pub fn new(paths: Paths, events: Events) -> Result<Arc<Self>> {
        let store = Arc::new(Store::open(paths)?);
        Ok(Arc::new(Self { store, events }))
    }

    pub fn paths(&self) -> &Paths {
        self.store.paths()
    }

    pub fn is_running(&self, _id: &str) -> bool {
        false
    }

    pub fn notify_instances_changed(&self) {
        events::emit(&*self.events, events::INSTANCES_CHANGED, &());
    }

    pub fn list_instances(&self) -> Vec<Instance> {
        self.store.list()
    }

    pub fn create_instance(&self, new: NewInstance) -> Result<Instance> {
        let inst = self.store.create(new)?;
        self.notify_instances_changed();
        Ok(inst)
    }

    pub fn update_instance(&self, inst: Instance) -> Result<Instance> {
        let inst = self.store.update(inst)?;
        self.notify_instances_changed();
        Ok(inst)
    }

    pub fn delete_instance(&self, id: &str) -> Result<()> {
        self.store.delete(id, self.is_running(id))?;
        self.notify_instances_changed();
        Ok(())
    }

    pub fn settings(&self) -> AppSettings {
        self.store.settings()
    }

    pub fn set_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        self.store.set_settings(settings)
    }
}
