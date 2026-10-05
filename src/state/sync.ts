/** Keeps the client stores in step with backend events. */
import { on } from "../lib/events";
import { appendConsole } from "./console";
import { applyPlayers, applyProgress, applySnapshot, refreshInstances, refreshPlayit, refreshSnapshots, setState } from "./store";

export function startSync(): () => void {
  const subs = [
    on("instances-changed", () => {
      refreshInstances();
      refreshSnapshots();
    }),
    on("instance-state", applySnapshot),
    on("metrics", (list) => list.forEach(applySnapshot)),
    on("players", ({ id, players }) => applyPlayers(id, players)),
    on("console-batch", ({ id, lines }) => appendConsole(id, lines)),
    on("task-progress", applyProgress),
    on("playit-state", (playit) => setState({ playit })),
  ];
  refreshInstances();
  refreshSnapshots();
  refreshPlayit();
  return () => {
    subs.forEach((p) => p.then((unlisten) => unlisten()).catch(() => {}));
  };
}
