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
  times: string[];
  mode: RestartMode;
}

export interface Instance {
  id: string;
  name: string;
  serverType: ServerType;
  mcVersion: string;
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
  maxPlayers?: number;
}

export interface AppSettings {
  theme: "dark" | "light";
  reduceEffects: boolean;
  closeToTray: boolean;
  startWithWindows: boolean;
  eulaAcceptedAt: string | null;
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
  joinInfo: (id: string) => invoke<JoinInfo>("join_info", { id }),

  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<AppSettings>("set_settings", { settings }),
  acceptEula: () => invoke<AppSettings>("accept_eula"),
};
