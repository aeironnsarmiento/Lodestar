import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { api, type Instance } from "../lib/api";
import { getState, loadSettings, refreshInstances, resetStore, saveSettings } from "../state/store";

const sample = { id: "speedrun", name: "Speedrun", serverType: "fabric", mcVersion: "26.3" } as Instance;

describe("api", () => {
  beforeEach(() => {
    invoke.mockReset();
    resetStore();
  });

  it("maps typed calls to backend command names and argument shapes", async () => {
    invoke.mockResolvedValue(sample);
    await api.createInstance({
      name: "Speedrun",
      serverType: "fabric",
      mcVersion: "26.3",
      gameMode: "survival",
      difficulty: "easy",
    });
    expect(invoke).toHaveBeenCalledWith("create_instance", {
      new: expect.objectContaining({ name: "Speedrun", serverType: "fabric", mcVersion: "26.3" }),
    });

    await api.deleteInstance("speedrun");
    expect(invoke).toHaveBeenLastCalledWith("delete_instance", { id: "speedrun" });
  });

  it("lists instances into the store and surfaces backend errors", async () => {
    invoke.mockResolvedValueOnce([sample]);
    await refreshInstances();
    expect(getState().instances).toEqual([sample]);

    invoke.mockRejectedValueOnce("Stop \"Speedrun\" before deleting it.");
    await refreshInstances();
    expect(getState().error).toContain("Stop");
  });

  it("saves settings optimistically and rolls back on failure", async () => {
    const settings = {
      theme: "dark" as const,
      reduceEffects: false,
      closeToTray: true,
      startWithWindows: false,
      eulaAcceptedAt: null,
    };
    invoke.mockResolvedValueOnce(settings);
    await loadSettings();

    invoke.mockImplementationOnce((_cmd: string, args: { settings: unknown }) => Promise.resolve(args.settings));
    await saveSettings({ theme: "light" });
    expect(invoke).toHaveBeenLastCalledWith("set_settings", { settings: { ...settings, theme: "light" } });
    expect(getState().settings?.theme).toBe("light");

    invoke.mockRejectedValueOnce("disk full");
    await saveSettings({ reduceEffects: true });
    expect(getState().settings?.reduceEffects).toBe(false);
    expect(getState().error).toBe("disk full");
  });
});
