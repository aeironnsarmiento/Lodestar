use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// App-wide settings stored in `settings.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub theme: Theme,
    pub reduce_effects: bool,
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    /// RFC 3339 time the Minecraft EULA was accepted (KTD15). `None` = not accepted.
    pub eula_accepted_at: Option<String>,
    /// The user's own CurseForge API key; CurseForge does not let apps ship one.
    pub curseforge_api_key: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            reduce_effects: false,
            close_to_tray: true,
            start_with_windows: false,
            eula_accepted_at: None,
            curseforge_api_key: String::new(),
        }
    }
}
