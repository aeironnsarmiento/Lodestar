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

  listVersions: (serverType: ServerType) => invoke<VersionEntry[]>("list_versions", { serverType }),

  listJavaRuntimes: () => invoke<JavaRuntime[]>("list_java_runtimes"),
  removeJavaRuntime: (major: number) => invoke<void>("remove_java_runtime", { major }),

  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<AppSettings>("set_settings", { settings }),
};
