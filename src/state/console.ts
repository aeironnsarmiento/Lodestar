/**
 * Console lines per instance, kept outside the main store so a busy console only
 * re-renders the console view.
 */
import { useSyncExternalStore } from "react";
import { api, type ConsoleLine } from "../lib/api";

const CAPACITY = 5000;
const EMPTY: ConsoleLine[] = [];

const lines = new Map<string, ConsoleLine[]>();
const listeners = new Map<string, Set<() => void>>();

function notify(id: string) {
  listeners.get(id)?.forEach((l) => l());
}

/** Appends live lines, skipping any already present (by sequence number). */
export function appendConsole(id: string, incoming: ConsoleLine[]): void {
  if (incoming.length === 0) return;
  const current = lines.get(id) ?? EMPTY;
  const last = current.length ? current[current.length - 1].seq : 0;
  const fresh = incoming.filter((l) => l.seq > last);
  if (fresh.length === 0) return;
  const next = current.concat(fresh);
  lines.set(id, next.length > CAPACITY ? next.slice(next.length - CAPACITY) : next);
  notify(id);
}

/** Replaces the buffer with the backend's snapshot. */
export function setConsole(id: string, snapshot: ConsoleLine[]): void {
  lines.set(id, snapshot.slice(-CAPACITY));
  notify(id);
}

export async function loadConsole(id: string): Promise<void> {
  try {
    const snapshot = await api.getConsole(id);
    // Keep live lines that arrived while the snapshot was in flight.
    const live = (lines.get(id) ?? EMPTY).filter((l) => l.seq > (snapshot.at(-1)?.seq ?? 0));
    setConsole(id, snapshot.concat(live));
  } catch {
    // Not running under Tauri, or the backend is unavailable.
  }
}

export function useConsole(id: string): ConsoleLine[] {
  return useSyncExternalStore(
    (cb) => {
      let set = listeners.get(id);
      if (!set) listeners.set(id, (set = new Set()));
      set.add(cb);
      return () => set!.delete(cb);
    },
    () => lines.get(id) ?? EMPTY,
  );
}

export function resetConsoles(): void {
  lines.clear();
  listeners.forEach((_, id) => notify(id));
}
