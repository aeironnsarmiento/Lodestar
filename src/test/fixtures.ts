import type { AppSettings, Instance, Snapshot } from "../lib/api";

export function makeInstance(overrides: Partial<Instance> = {}): Instance {
  return {
    id: "speedrun",
    name: "Speedrun",
    serverType: "fabric",
    mcVersion: "26.3",
    launch: { kind: "jar", jar: "C:\\cache\\fabric.jar" },
    javaMajor: 25,
    currentWorld: null,
    initialSeed: null,
    ramMb: 8192,
    port: 25565,
    difficulty: "easy",
    gameMode: "survival",
    hardcore: false,
    maxPlayers: 10,
    viewDistance: 16,
    simulationDistance: 8,
    motd: "A Lodestar server",
    onlineMode: true,
    opName: "",
    operators: [],
    whitelist: [],
    properties: {
      pvp: true,
      allowNether: true,
      generateStructures: true,
      levelType: "normal",
      spawnProtection: 0,
      forceGamemode: false,
      enableCommandBlock: false,
      allowFlight: true,
      playerIdleTimeout: 0,
      enforceSecureProfile: false,
      hideOnlinePlayers: false,
      whiteList: false,
      enforceWhitelist: false,
      syncChunkWrites: false,
      entityBroadcastRangePercentage: 100,
      resourcePack: "",
      requireResourcePack: false,
    },
    autoStart: false,
    restart: { times: [], mode: "warn" },
    speedMods: true,
    managedMods: [],
    managedModsFor: null,
    provision: { state: "ready" },
    createdAt: "2026-10-05T10:00:00+00:00",
    ...overrides,
  };
}

export function makeSnapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    id: "speedrun",
    state: "stopped",
    port: 25565,
    pid: null,
    players: [],
    cpuPercent: 0,
    memoryBytes: 0,
    uptimeSecs: null,
    message: null,
    ...overrides,
  };
}

export const settings: AppSettings = {
  theme: "dark",
  reduceEffects: false,
  closeToTray: true,
  startWithWindows: false,
  eulaAcceptedAt: null,
};

export const versions = [
  { id: "26.4-snapshot-2", kind: "snapshot" as const, releaseTime: "2026-09-29T11:32:57+00:00" },
  { id: "26.3", kind: "release" as const, releaseTime: "2026-09-15T11:23:02+00:00" },
  { id: "26.2", kind: "release" as const, releaseTime: "2026-06-16T09:00:00+00:00" },
  { id: "1.17.1", kind: "release" as const, releaseTime: "2021-07-06T12:01:34+00:00" },
  { id: "1.16.5", kind: "release" as const, releaseTime: "2021-01-14T16:05:32+00:00" },
];
