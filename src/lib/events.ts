/** Typed wrappers over backend events (KTD3). */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface EventPayloads {
  "instances-changed": null;
}

export type EventName = keyof EventPayloads;

export function on<E extends EventName>(event: E, handler: (payload: EventPayloads[E]) => void): Promise<UnlistenFn> {
  return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
}
