import type { ServerType, VersionEntry } from "./api";

function key(v: string): number[] {
  const core = v.split(/[-_+ ]/)[0];
  const out: number[] = [];
  for (const part of core.split(".")) {
    const n = Number(part);
    if (!Number.isInteger(n)) break;
    out.push(n);
  }
  return out;
}

/** Numeric comparison of Minecraft versions (year versions like 26.3 sort above 1.x). */
export function compareVersions(a: string, b: string): number {
  const ka = key(a);
  const kb = key(b);
  for (let i = 0; i < Math.max(ka.length, kb.length); i++) {
    const d = (ka[i] ?? 0) - (kb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Forge and NeoForge are supported from Minecraft 1.17 on. */
export function supportsVersion(type: ServerType, version: string): boolean {
  if (type === "forge" || type === "neoforge") return key(version).length > 0 && compareVersions(version, "1.17") >= 0;
  return true;
}

export function visibleVersions(type: ServerType, list: VersionEntry[], showSnapshots: boolean): VersionEntry[] {
  return list.filter((v) => (showSnapshots || v.kind === "release") && supportsVersion(type, v.id));
}
