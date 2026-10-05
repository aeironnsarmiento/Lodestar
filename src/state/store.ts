/**
 * A small client-side mirror of backend state. Components read it with `useStore`;
 * it is filled from commands and kept fresh by backend events.
 */
import { useSyncExternalStore } from "react";
import { api, type AppSettings, type Instance, type Snapshot, type TaskProgress } from "../lib/api";

export interface StoreState {
  instances: Instance[];
  /** Live state per instance id. */
  runtime: Record<string, Snapshot>;
  /** Latest download/setup progress per instance id. */
  progress: Record<string, TaskProgress>;
  settings: AppSettings | null;
  error: string | null;
  /** Instance waiting on the EULA dialog before it can launch. */
  eulaPrompt: string | null;
  /** Last failed action per instance (launch, stop...). */
  actionErrors: Record<string, string>;
}

type Listener = () => void;

const initial = (): StoreState => ({
  instances: [],
  runtime: {},
  progress: {},
  settings: null,
  error: null,
  eulaPrompt: null,
  actionErrors: {},
});

let state: StoreState = initial();
const listeners = new Set<Listener>();

export function getState(): StoreState {
  return state;
}

export function setState(patch: Partial<StoreState> | ((s: StoreState) => Partial<StoreState>)): void {
  const next = typeof patch === "function" ? patch(state) : patch;
  state = { ...state, ...next };
  listeners.forEach((l) => l());
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useStore<T>(select: (s: StoreState) => T): T {
  return useSyncExternalStore(subscribe, () => select(state));
}

/** Test helper: back to an empty store. */
export function resetStore(): void {
  state = initial();
  listeners.forEach((l) => l());
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export async function refreshInstances(): Promise<void> {
  try {
    setState({ instances: await api.listInstances(), error: null });
  } catch (e) {
    setState({ error: errorMessage(e) });
  }
}

/** What a never-started instance looks like. */
export function stoppedSnapshot(id: string, port = 0): Snapshot {
  return { id, state: "stopped", port, pid: null, players: [], cpuPercent: 0, memoryBytes: 0, uptimeSecs: null, message: null };
}

export function applySnapshot(snapshot: Snapshot): void {
  setState((s) => ({ runtime: { ...s.runtime, [snapshot.id]: snapshot } }));
}

export function applyPlayers(id: string, players: string[]): void {
  setState((s) => {
    const current = s.runtime[id];
    return current ? { runtime: { ...s.runtime, [id]: { ...current, players } } } : {};
  });
}

export function applyProgress(progress: TaskProgress): void {
  setState((s) => ({ progress: { ...s.progress, [progress.task]: progress } }));
}

export async function refreshSnapshots(): Promise<void> {
  try {
    const list = await api.serverSnapshots();
    setState({ runtime: Object.fromEntries(list.map((x) => [x.id, x])) });
  } catch (e) {
    setState({ error: errorMessage(e) });
  }
}

export async function loadSettings(): Promise<AppSettings> {
  const settings = await api.getSettings();
  setState({ settings });
  return settings;
}

export async function saveSettings(patch: Partial<AppSettings>): Promise<AppSettings | null> {
  const current = state.settings;
  if (!current) return null;
  const optimistic = { ...current, ...patch };
  setState({ settings: optimistic });
  try {
    const saved = await api.setSettings(optimistic);
    setState({ settings: saved });
    return saved;
  } catch (e) {
    setState({ settings: current, error: errorMessage(e) });
    return null;
  }
}
