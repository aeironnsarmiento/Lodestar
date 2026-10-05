/**
 * Typed wrappers over the backend's Tauri commands (KTD3). The backend owns all
 * state; the frontend calls these and listens to events (see events.ts).
 */
import { invoke } from "@tauri-apps/api/core";

export type ServerType = "vanilla" | "paper" | "fabric" | "forge" | "neoforge";
export type GameMode = "survival" | "creative" | "adventure" | "spectator";
export type Difficulty = "peaceful" | "easy" | "normal" | "hard";
export type RestartMode = "warn" | "postpone";

export type Provision =
  | { state: "pending" }
  | { state: "running"; message: string }
  | { state: "ready" }
  | { state: "failed"; message: string };

export type LaunchInfo = { kind: "jar"; jar: string } | { kind: "argsFile"; argsFile: string };

export interface RestartSchedule {
  enabled: boolean;
  times: string[];
  mode: RestartMode;
}

export type LevelType = "normal" | "flat" | "largeBiomes" | "amplified";

/** server.properties keys managed by the app (besides the core ones on Instance). */
export interface ServerProperties {
  pvp: boolean;
  allowNether: boolean;
  generateStructures: boolean;
  levelType: LevelType;
  spawnProtection: number;
  forceGamemode: boolean;
  enableCommandBlock: boolean;
  allowFlight: boolean;
  playerIdleTimeout: number;
  enforceSecureProfile: boolean;
  hideOnlinePlayers: boolean;
  whiteList: boolean;
  enforceWhitelist: boolean;
  syncChunkWrites: boolean;
  entityBroadcastRangePercentage: number;
  resourcePack: string;
  requireResourcePack: boolean;
}

export type AddonSource = "modrinth" | "curseforge";
export type ProjectKind = "mod" | "plugin" | "modpack";

export interface MissingFile {
  fileName: string;
  title: string;
  pageUrl: string;
}

/** The modpack a server was made from. */
export interface ModpackRef {
  source: AddonSource | null;
  projectId: string | null;
  versionId: string | null;
  title: string;
  versionNumber: string | null;
  iconUrl: string | null;
  pageUrl: string | null;
  file: string | null;
  installed: boolean;
  missing: MissingFile[];
}

export interface Instance {
  id: string;
  name: string;
  serverType: ServerType;
  mcVersion: string;
  loaderVersion: string | null;
  modpack: ModpackRef | null;
  launch: LaunchInfo | null;
  javaMajor: number | null;
  currentWorld: string | null;
  initialSeed: string | null;
  ramMb: number;
  port: number;
  difficulty: Difficulty;
  gameMode: GameMode;
  hardcore: boolean;
  maxPlayers: number;
  viewDistance: number;
  simulationDistance: number;
  motd: string;
  onlineMode: boolean;
  opName: string;
  operators: string[];
  whitelist: string[];
  properties: ServerProperties;
  autoStart: boolean;
  restart: RestartSchedule;
  speedMods: boolean;
  managedMods: string[];
  managedModsFor: string | null;
  provision: Provision;
  createdAt: string;
}

export interface NewInstance {
  name: string;
  serverType: ServerType;
  mcVersion: string;
  seed?: string | null;
  gameMode: GameMode;
  difficulty: Difficulty;
  hardcore?: boolean;
  maxPlayers?: number;
  loaderVersion?: string | null;
  modpack?: Partial<ModpackRef> | null;
}

export interface AppSettings {
  theme: "dark" | "light";
  reduceEffects: boolean;
  closeToTray: boolean;
  startWithWindows: boolean;
  eulaAcceptedAt: string | null;
  curseforgeApiKey: string;
}

/** One file in a server's mods or plugins folder. */
export interface AddonEntry {
  fileName: string;
  enabled: boolean;
  size: number;
  source: AddonSource | null;
  projectId: string | null;
  versionId: string | null;
  title: string | null;
  versionNumber: string | null;
  iconUrl: string | null;
  pageUrl: string | null;
  fromModpack: boolean;
  unidentified: boolean;
  sha1: string | null;
}

export interface SearchHit {
  source: AddonSource;
  kind: ProjectKind;
  projectId: string;
  slug: string;
  title: string;
  author: string;
  description: string;
  iconUrl: string | null;
  downloads: number;
  pageUrl: string;
}

export interface SearchPage {
  hits: SearchHit[];
  total: number;
}

export interface ProjectVersion {
  source: AddonSource;
  projectId: string;
  versionId: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  channel: "release" | "beta" | "alpha";
  published: string;
  fileName: string;
}

export interface BlockedFile {
  title: string;
  fileName: string;
  pageUrl: string;
}

export interface InstallResult {
  installed: string[];
  blocked: BlockedFile[];
  notes: string[];
}

export interface AddonUpdate {
  fileName: string;
  versionId: string;
  versionNumber: string;
}

export interface ImportResult {
  added: string[];
  skipped: string[];
}

export interface PackInfo {
  format: AddonSource;
  name: string;
  version: string;
  mcVersion: string;
  serverType: ServerType;
  loaderVersion: string | null;
}

export interface VersionEntry {
  id: string;
  kind: "release" | "snapshot";
  releaseTime: string | null;
}

export interface JavaRuntime {
  major: number;
  path: string;
  sizeBytes: number;
  inUse: boolean;
}

export type ServerState = "stopped" | "preparing" | "starting" | "online" | "stopping" | "crashed";

export interface Snapshot {
  id: string;
  state: ServerState;
  port: number;
  pid: number | null;
  players: string[];
  cpuPercent: number;
  memoryBytes: number;
  uptimeSecs: number | null;
  message: string | null;
}

export interface ConsoleLine {
  seq: number;
  text: string;
  history: boolean;
}

export interface JoinInfo {
  localhost: string;
  lan: string | null;
  public: string | null;
}

export interface TaskProgress {
  task: string;
  label: string;
  done: number;
  total: number | null;
}

export interface WorldInfo {
  name: string;
  seed: string | null;
  createdAt: string | null;
  sizeBytes: number;
  current: boolean;
}

export type LinkState = "notSetUp" | "installing" | "waitingForClaim" | "agentOffline" | "linked";
export type TunnelState = "pending" | "connected" | "limitReached" | "error";

export interface TunnelStatus {
  port: number;
  state: TunnelState;
  address: string | null;
  message: string | null;
}

export interface PlayitStatus {
  state: LinkState;
  claimUrl: string | null;
  message: string | null;
  tunnels: TunnelStatus[];
}

export const SERVER_TYPE_LABELS: Record<ServerType, string> = {
  vanilla: "Vanilla",
  paper: "Paper",
  fabric: "Fabric",
  forge: "Forge",
  neoforge: "NeoForge",
};

/** True inside the Tauri webview; false in tests and a plain browser. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const api = {
  listInstances: () => invoke<Instance[]>("list_instances"),
  createInstance: (newInstance: NewInstance) => invoke<Instance>("create_instance", { new: newInstance }),
  updateInstance: (instance: Instance) => invoke<Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),

  retryProvision: (id: string) => invoke<void>("retry_provision", { id }),
  openAddonsFolder: (id: string) => invoke<void>("open_addons_folder", { id }),
  listVersions: (serverType: ServerType) => invoke<VersionEntry[]>("list_versions", { serverType }),

  listJavaRuntimes: () => invoke<JavaRuntime[]>("list_java_runtimes"),
  removeJavaRuntime: (major: number) => invoke<void>("remove_java_runtime", { major }),

  startServer: (id: string) => invoke<void>("start_server", { id }),
  stopServer: (id: string) => invoke<void>("stop_server", { id }),
  restartServer: (id: string) => invoke<void>("restart_server", { id }),
  killServer: (id: string) => invoke<void>("kill_server", { id }),
  sendCommand: (id: string, command: string) => invoke<void>("send_command", { id, command }),
  getConsole: (id: string) => invoke<ConsoleLine[]>("get_console", { id }),
  serverSnapshots: () => invoke<Snapshot[]>("server_snapshots"),
  listWorlds: (id: string) => invoke<WorldInfo[]>("list_worlds", { id }),
  resetWorld: (id: string, seed: string | null) => invoke<string>("reset_world", { id, seed }),
  switchWorld: (id: string, name: string) => invoke<void>("switch_world", { id, name }),
  joinInfo: (id: string) => invoke<JoinInfo>("join_info", { id }),

  playitStatus: () => invoke<PlayitStatus>("playit_status"),
  playitSetup: () => invoke<string>("playit_setup"),
  playitCancel: () => invoke<void>("playit_cancel"),
  playitRelink: () => invoke<string>("playit_relink"),
  playitRetryTunnel: (port: number) => invoke<void>("playit_retry_tunnel", { port }),

  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<AppSettings>("set_settings", { settings }),
  acceptEula: () => invoke<AppSettings>("accept_eula"),
  quitApp: () => invoke<void>("quit_app"),

  listAddons: (id: string) => invoke<AddonEntry[]>("list_addons", { id }),
  identifyAddons: (id: string) => invoke<AddonEntry[]>("identify_addons", { id }),
  setAddonEnabled: (id: string, fileName: string, enabled: boolean) => invoke<void>("set_addon_enabled", { id, fileName, enabled }),
  removeAddon: (id: string, fileName: string) => invoke<void>("remove_addon", { id, fileName }),
  importAddons: (id: string, paths: string[]) => invoke<ImportResult>("import_addons", { id, paths }),
  searchProjects: (source: AddonSource, kind: ProjectKind, text: string, gameVersion: string | null, loaders: string[], offset: number) =>
    invoke<SearchPage>("search_projects", { source, kind, text, gameVersion, loaders, offset }),
  projectVersions: (source: AddonSource, projectId: string, loaders: string[], gameVersion: string | null) =>
    invoke<ProjectVersion[]>("project_versions", { source, projectId, loaders, gameVersion }),
  installProject: (id: string, source: AddonSource, projectId: string, versionId: string | null) =>
    invoke<InstallResult>("install_project", { id, source, projectId, versionId }),
  checkAddonUpdates: (id: string) => invoke<AddonUpdate[]>("check_addon_updates", { id }),
  updateAddon: (id: string, fileName: string, versionId: string) => invoke<InstallResult>("update_addon", { id, fileName, versionId }),
  inspectModpackFile: (path: string) => invoke<PackInfo>("inspect_modpack_file", { path }),
  updateModpack: (id: string, versionId: string, versionNumber: string | null) =>
    invoke<void>("update_modpack", { id, versionId, versionNumber }),
};

/** Loader names (Modrinth spelling) a server type can run. */
export function loadersFor(serverType: ServerType): string[] {
  switch (serverType) {
    case "paper":
      return ["paper", "spigot", "bukkit", "purpur"];
    case "fabric":
      return ["fabric"];
    case "forge":
      return ["forge"];
    case "neoforge":
      return ["neoforge"];
    default:
      return [];
  }
}

/** What a server type's add-ons are called, or null when it cannot load any. */
export function addonKindFor(serverType: ServerType): "mod" | "plugin" | null {
  if (serverType === "vanilla") return null;
  return serverType === "paper" ? "plugin" : "mod";
}

export const SOURCE_LABELS: Record<AddonSource, string> = { modrinth: "Modrinth", curseforge: "CurseForge" };
