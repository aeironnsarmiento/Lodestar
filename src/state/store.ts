/**
 * A small client-side mirror of backend state. Components read it with `useStore`;
 * it is filled from commands and kept fresh by backend events.
 */
import { useSyncExternalStore } from "react";
import { api, type AppSettings, type Instance } from "../lib/api";

export interface StoreState {
  instances: Instance[];
  settings: AppSettings | null;
  error: string | null;
}

type Listener = () => void;

let state: StoreState = { instances: [], settings: null, error: null };
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
  state = { instances: [], settings: null, error: null };
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
