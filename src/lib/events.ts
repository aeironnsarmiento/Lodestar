/** Typed wrappers over backend events (KTD3). */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ConsoleLine, PlayitStatus, Snapshot, TaskProgress } from "./api";

export interface EventPayloads {
  "instances-changed": null;
  "instance-state": Snapshot;
  "console-batch": { id: string; lines: ConsoleLine[] };
  metrics: Snapshot[];
  players: { id: string; players: string[] };
  "task-progress": TaskProgress;
  "playit-state": PlayitStatus;
}

export type EventName = keyof EventPayloads;

export function on<E extends EventName>(event: E, handler: (payload: EventPayloads[E]) => void): Promise<UnlistenFn> {
  return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
}
