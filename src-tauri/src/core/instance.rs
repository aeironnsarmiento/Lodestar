//! The persisted instance record (`instances/<id>/instance.json`).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    #[default]
    Vanilla,
    Paper,
    Fabric,
    Forge,
    Neoforge,
}

impl ServerType {
    pub fn label(self) -> &'static str {
        match self {
            ServerType::Vanilla => "Vanilla",
            ServerType::Paper => "Paper",
            ServerType::Fabric => "Fabric",
            ServerType::Forge => "Forge",
            ServerType::Neoforge => "NeoForge",
        }
    }

    /// The folder a modded type loads add-ons from (R25). Vanilla has none.
    pub fn addons_folder(self) -> Option<&'static str> {
        match self {
            ServerType::Vanilla => None,
            ServerType::Paper => Some("plugins"),
            ServerType::Fabric | ServerType::Forge | ServerType::Neoforge => Some("mods"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn as_str(self) -> &'static str {
        match self {
            GameMode::Survival => "survival",
            GameMode::Creative => "creative",
            GameMode::Adventure => "adventure",
            GameMode::Spectator => "spectator",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Peaceful,
    #[default]
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn as_str(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "peaceful",
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RestartMode {
    /// In-game countdown warnings, then restart on time.
    #[default]
    Warn,
    /// If players are online at the due time, wait until the server is empty.
    Postpone,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RestartSchedule {
    /// Daily local times, `HH:MM`.
    pub times: Vec<String>,
    pub mode: RestartMode,
}

/// How to launch the installed server, decided at provisioning time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaunchInfo {
    /// `java -jar <jar> nogui`. The path is absolute (shared jar cache or the server folder).
    #[serde(rename_all = "camelCase")]
    Jar { jar: String },
    /// Forge/NeoForge: `java @user_jvm_args.txt @<argsFile> nogui`, relative to the server folder.
    #[serde(rename_all = "camelCase")]
    ArgsFile { args_file: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Provision {
    #[default]
    Pending,
    Running { message: String },
    Ready,
    Failed { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    pub mc_version: String,
    pub launch: Option<LaunchInfo>,
    /// Java feature version this instance runs on (resolved from Mojang metadata, KTD9).
    pub java_major: Option<u32>,
    /// Name of the run folder under `server/worlds/` that the next start uses.
    pub current_world: Option<String>,
    /// Seed chosen at creation; used for the first world only.
    pub initial_seed: Option<String>,
    pub ram_mb: u32,
    pub port: u16,
    pub difficulty: Difficulty,
    pub game_mode: GameMode,
    pub hardcore: bool,
    pub max_players: u32,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub motd: String,
    pub online_mode: bool,
    /// The host's Minecraft name; opped when the server comes online.
    pub op_name: String,
    pub auto_start: bool,
    pub restart: RestartSchedule,
    pub speed_mods: bool,
    pub managed_mods: Vec<String>,
    /// `<type>-<version>` the managed mods were installed for.
    pub managed_mods_for: Option<String>,
    pub provision: Provision,
    pub created_at: String,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            server_type: ServerType::Vanilla,
            mc_version: String::new(),
            launch: None,
            java_major: None,
            current_world: None,
            initial_seed: None,
            ram_mb: 4096,
            port: 25565,
            difficulty: Difficulty::Easy,
            game_mode: GameMode::Survival,
            hardcore: false,
            max_players: 10,
            view_distance: 10,
            simulation_distance: 10,
            motd: "A Lodestar server".into(),
            online_mode: true,
            op_name: String::new(),
            auto_start: false,
            restart: RestartSchedule::default(),
            speed_mods: true,
            managed_mods: Vec::new(),
            managed_mods_for: None,
            provision: Provision::Pending,
            created_at: String::new(),
        }
    }
}

/// What the New Server dialog sends (R1).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NewInstance {
    pub name: String,
    pub server_type: ServerType,
    pub mc_version: String,
    pub seed: Option<String>,
    pub game_mode: GameMode,
    pub difficulty: Difficulty,
    pub max_players: Option<u32>,
}

/// Lower-case, dash-separated folder name from a display name.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "server".into()
    } else {
        out.chars().take(40).collect()
    }
}
