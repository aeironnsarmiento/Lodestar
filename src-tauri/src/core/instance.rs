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

/// World generator for new worlds. Written with the legacy lower-case names, which
/// every supported version (1.17 and later) accepts.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LevelType {
    #[default]
    Normal,
    Flat,
    LargeBiomes,
    Amplified,
}

impl LevelType {
    pub fn as_str(self) -> &'static str {
        match self {
            LevelType::Normal => "default",
            LevelType::Flat => "flat",
            LevelType::LargeBiomes => "largebiomes",
            LevelType::Amplified => "amplified",
        }
    }
}

/// The `server.properties` keys Lodestar manages beyond the core ones on [`Instance`].
/// Defaults favour a friendly private server (no spawn protection, flight allowed so
/// lag does not kick players, unsigned chat accepted).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerProperties {
    pub pvp: bool,
    pub allow_nether: bool,
    pub generate_structures: bool,
    pub level_type: LevelType,
    pub spawn_protection: u32,
    pub force_gamemode: bool,
    pub enable_command_block: bool,
    pub allow_flight: bool,
    /// Minutes before idle players are kicked; 0 never kicks.
    pub player_idle_timeout: u32,
    pub enforce_secure_profile: bool,
    pub hide_online_players: bool,
    pub white_list: bool,
    pub enforce_whitelist: bool,
    pub sync_chunk_writes: bool,
    pub entity_broadcast_range_percentage: u32,
    pub resource_pack: String,
    pub require_resource_pack: bool,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            pvp: true,
            allow_nether: true,
            generate_structures: true,
            level_type: LevelType::Normal,
            spawn_protection: 0,
            force_gamemode: false,
            enable_command_block: false,
            allow_flight: true,
            player_idle_timeout: 0,
            enforce_secure_profile: false,
            hide_online_players: false,
            white_list: false,
            enforce_whitelist: false,
            sync_chunk_writes: false,
            entity_broadcast_range_percentage: 100,
            resource_pack: String::new(),
            require_resource_pack: false,
        }
    }
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
    /// Further operators, opped when the server comes online.
    pub operators: Vec<String>,
    /// Players allowed when the whitelist is on; added when the server comes online.
    pub whitelist: Vec<String>,
    pub properties: ServerProperties,
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
            ram_mb: 8192,
            port: 25565,
            difficulty: Difficulty::Easy,
            game_mode: GameMode::Survival,
            hardcore: false,
            max_players: 10,
            view_distance: 16,
            simulation_distance: 8,
            motd: "A Lodestar server".into(),
            online_mode: true,
            op_name: String::new(),
            operators: Vec::new(),
            whitelist: Vec::new(),
            properties: ServerProperties::default(),
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
    pub hardcore: bool,
    pub max_players: Option<u32>,
}

/// A Minecraft player name as typed: trimmed, 1–16 letters, digits or underscores.
pub fn is_player_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Trims, drops invalid names and case-insensitive duplicates, keeping the first spelling.
pub fn clean_player_list(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names.iter().map(|n| n.trim()) {
        if is_player_name(name) && !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
            out.push(name.to_string());
        }
    }
    out
}

impl Instance {
    /// Keeps values inside the ranges Minecraft accepts.
    pub fn normalize(&mut self) {
        self.operators = clean_player_list(&self.operators);
        self.whitelist = clean_player_list(&self.whitelist);
        self.op_name = self.op_name.trim().to_string();
        if self.hardcore {
            self.difficulty = Difficulty::Hard;
        }
        let p = &mut self.properties;
        p.spawn_protection = p.spawn_protection.min(1000);
        p.player_idle_timeout = p.player_idle_timeout.min(1440);
        p.entity_broadcast_range_percentage = p.entity_broadcast_range_percentage.clamp(10, 1000);
        p.resource_pack = p.resource_pack.trim().to_string();
    }
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
