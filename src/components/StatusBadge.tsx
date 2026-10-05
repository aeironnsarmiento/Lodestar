import type { ServerState } from "../lib/api";
import { GlassPill } from "./glass/GlassPill";

const LABELS: Record<ServerState | "setup" | "setupFailed", string> = {
  stopped: "Stopped",
  preparing: "Preparing",
  starting: "Starting",
  online: "Online",
  stopping: "Stopping",
  crashed: "Crashed",
  setup: "Setting up",
  setupFailed: "Setup failed",
};

export type BadgeState = ServerState | "setup" | "setupFailed";

export function StatusBadge({ state }: { state: BadgeState }) {
  return (
    <GlassPill className={`status status-${state}`} data-state={state}>
      <span className="dot" aria-hidden="true" />
      {LABELS[state]}
    </GlassPill>
  );
}
