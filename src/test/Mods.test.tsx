import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(() => Promise.resolve(null)) }));

import { ModsTab } from "../pages/server/ModsTab";
import { NewServerDialog } from "../dialogs/NewServerDialog";
import type { AddonEntry, ProjectVersion, SearchHit } from "../lib/api";
import { resetStore } from "../state/store";
import { makeInstance, versions } from "./fixtures";

function entry(overrides: Partial<AddonEntry> = {}): AddonEntry {
  return {
    fileName: "lithium.jar",
    enabled: true,
    size: 2048,
    source: "modrinth",
    projectId: "lithium",
    versionId: "v1",
    title: "Lithium",
    versionNumber: "0.15.0",
    iconUrl: null,
    pageUrl: "https://modrinth.com/mod/lithium",
    fromModpack: false,
    unidentified: false,
    sha1: null,
    ...overrides,
  };
}

const hit: SearchHit = {
  source: "modrinth",
  kind: "mod",
  projectId: "servercore",
  slug: "servercore",
  title: "ServerCore",
  author: "Wesley",
  description: "Server performance",
  iconUrl: null,
  downloads: 1_500_000,
  pageUrl: "https://modrinth.com/mod/servercore",
};

const version: ProjectVersion = {
  source: "modrinth",
  projectId: "servercore",
  versionId: "sc-2",
  name: "ServerCore 1.2",
  versionNumber: "1.2.0",
  gameVersions: ["26.3"],
  loaders: ["fabric"],
  channel: "release",
  published: "2026-09-01",
  fileName: "servercore-1.2.0.jar",
};

describe("Mods tab", () => {
  beforeEach(() => {
    invoke.mockReset();
    resetStore();
  });

  it("lists mods, turns one off, removes one after confirming, and updates", async () => {
    const user = userEvent.setup();
    let list = [entry(), entry({ fileName: "hand.jar", source: null, projectId: null, title: null, versionNumber: null, unidentified: true })];
    invoke.mockImplementation((cmd: string, args?: any) => {
      if (cmd === "list_addons") return Promise.resolve(list);
      if (cmd === "set_addon_enabled") {
        list = list.map((e) => (e.fileName === args.fileName ? { ...e, enabled: args.enabled } : e));
        return Promise.resolve(null);
      }
      if (cmd === "remove_addon") {
        list = list.filter((e) => e.fileName !== args.fileName);
        return Promise.resolve(null);
      }
      if (cmd === "check_addon_updates") return Promise.resolve([{ fileName: "lithium.jar", versionId: "v2", versionNumber: "0.16.0" }]);
      if (cmd === "update_addon") return Promise.resolve({ installed: ["Lithium"], blocked: [], notes: [] });
      return Promise.resolve(null);
    });
    render(<ModsTab instance={makeInstance({ serverType: "fabric" })} running={false} />);

    const lithium = (await screen.findByText("Lithium")).closest("li")!;
    expect(screen.getByText("hand")).toBeInTheDocument();
    expect(screen.getByText("Added by hand")).toBeInTheDocument();

    await user.click(within(lithium).getByRole("switch", { name: "Lithium enabled" }));
    expect(invoke).toHaveBeenCalledWith("set_addon_enabled", { id: "speedrun", fileName: "lithium.jar", enabled: false });

    await user.click(screen.getByRole("button", { name: /check for updates/i }));
    await user.click(await screen.findByRole("button", { name: "Update to 0.16.0" }));
    expect(invoke).toHaveBeenCalledWith("update_addon", { id: "speedrun", fileName: "lithium.jar", versionId: "v2" });

    await user.click(screen.getByRole("button", { name: "Remove hand" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(invoke).toHaveBeenCalledWith("remove_addon", { id: "speedrun", fileName: "hand.jar" });
    await waitFor(() => expect(screen.queryByText("hand")).not.toBeInTheDocument());
  });

  it("locks changes to files while the server runs", async () => {
    invoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "list_addons" ? [entry()] : null));
    render(<ModsTab instance={makeInstance({ serverType: "fabric" })} running />);
    expect(await screen.findByRole("switch", { name: "Lithium enabled" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Remove Lithium" })).toBeDisabled();
  });

  it("browses Modrinth for mods that fit the server and installs one", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "list_addons") return Promise.resolve([]);
      if (cmd === "search_projects") return Promise.resolve({ hits: [hit], total: 1 });
      if (cmd === "project_versions") return Promise.resolve([version]);
      if (cmd === "install_project") return Promise.resolve({ installed: ["ServerCore", "Fabric API"], blocked: [], notes: [] });
      return Promise.resolve(null);
    });
    render(<ModsTab instance={makeInstance({ serverType: "fabric" })} running={false} />);
    await user.click(await screen.findByRole("button", { name: /browse mods/i }));

    const dialog = await screen.findByRole("dialog", { name: "Browse mods" });
    await within(dialog).findByRole("option", { name: /ServerCore/ });
    expect(invoke).toHaveBeenCalledWith("search_projects", {
      source: "modrinth",
      kind: "mod",
      text: "",
      gameVersion: "26.3",
      loaders: ["fabric"],
      offset: 0,
    });
    await within(dialog).findByText("1.2.0");
    await user.click(within(dialog).getByRole("button", { name: "Install latest" }));
    expect(invoke).toHaveBeenCalledWith("install_project", { id: "speedrun", source: "modrinth", projectId: "servercore", versionId: null });
    expect(await within(dialog).findByText(/Installed ServerCore, Fabric API/)).toBeInTheDocument();
  });
});

describe("New server from a modpack", () => {
  beforeEach(() => {
    invoke.mockReset();
    resetStore();
  });

  it("picks a pack version online and creates a server pinned to it", async () => {
    const user = userEvent.setup();
    const pack: SearchHit = { ...hit, kind: "modpack", projectId: "cobblemon", slug: "cobblemon", title: "Cobblemon Pack" };
    invoke.mockImplementation((cmd: string, args?: any) => {
      if (cmd === "list_versions") return Promise.resolve(versions);
      if (cmd === "search_projects") return Promise.resolve({ hits: [pack], total: 1 });
      if (cmd === "project_versions") return Promise.resolve([{ ...version, projectId: "cobblemon", versionId: "cp-9", versionNumber: "9.0", loaders: ["fabric"], gameVersions: ["1.21.1"] }]);
      if (cmd === "create_instance") return Promise.resolve(makeInstance({ name: args.new.name }));
      return Promise.resolve(null);
    });
    const onCreated = vi.fn();
    render(<NewServerDialog onClose={() => {}} onCreated={onCreated} />);

    await user.click(screen.getByRole("radio", { name: "A modpack" }));
    await user.click(screen.getByRole("button", { name: /browse modpacks/i }));
    const browser = await screen.findByRole("dialog", { name: "Choose a modpack" });
    await within(browser).findByText("9.0");
    await user.click(within(browser).getByRole("button", { name: "Use this" }));

    expect(screen.getByLabelText("Name")).toHaveValue("Cobblemon Pack");
    await user.click(screen.getByRole("button", { name: /create server/i }));
    expect(invoke).toHaveBeenCalledWith("create_instance", {
      new: expect.objectContaining({
        serverType: "fabric",
        mcVersion: "1.21.1",
        modpack: expect.objectContaining({ source: "modrinth", projectId: "cobblemon", versionId: "cp-9" }),
      }),
    });
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
  });
});
