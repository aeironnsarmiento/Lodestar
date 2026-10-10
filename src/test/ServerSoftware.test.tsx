import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { SettingsTab } from "../pages/server/SettingsTab";
import type { LoaderChoices } from "../lib/api";
import { makeInstance } from "./fixtures";

const tsp = { fileName: "TizioSpaceProject.jar", modName: "Tizio Space Project", range: "[47.4.20,)", summary: "47.4.20 or newer" };
const choices: LoaderChoices = {
  versions: [
    { id: "47.4.26", tag: "latest", compatible: true, rejectedBy: [] },
    { id: "47.4.20", tag: null, compatible: true, rejectedBy: [] },
    { id: "47.4.10", tag: "recommended", compatible: false, rejectedBy: [0] },
  ],
  requirements: [tsp],
  automatic: "47.4.26",
  installed: "47.4.10",
};

const forge = makeInstance({
  serverType: "forge",
  mcVersion: "1.20.1",
  launch: { kind: "argsFile", argsFile: "libraries/net/minecraftforge/forge/1.20.1-47.4.10/win_args.txt" },
});

describe("Server software settings", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => (cmd === "loader_choices" ? Promise.resolve(choices) : Promise.resolve(undefined)));
  });

  it("warns that a mod refuses the installed Forge and reinstalls the picked build", async () => {
    const user = userEvent.setup();
    render(<SettingsTab instance={forge} running={false} onDeleted={() => {}} />);
    expect(await screen.findByText(/Tizio Space Project needs Forge 47.4.20 or newer, so the server will not start on 47.4.10/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("loader_choices", { serverType: "forge", mcVersion: "1.20.1", id: "speedrun" });

    const select = screen.getByLabelText("Forge version");
    expect(within(select).getByText("Automatic (47.4.26)")).toBeInTheDocument();
    expect(within(select).getByText("47.4.10 (recommended, installed, a mod needs another version)")).toBeInTheDocument();

    // Automatic already moves off the refused build.
    expect(screen.getByRole("button", { name: "Install 47.4.26" })).toBeEnabled();
    await user.selectOptions(select, "47.4.20");
    await user.click(screen.getByRole("button", { name: "Install 47.4.20" }));
    expect(invoke).toHaveBeenCalledWith("set_loader_version", { id: "speedrun", version: "47.4.20" });

    // Picking the refused build says why.
    await user.selectOptions(select, "47.4.10");
    expect(screen.getByText("Tizio Space Project needs Forge 47.4.20 or newer.")).toBeInTheDocument();
  });

  it("cannot reinstall while the server runs, and Vanilla has no panel", async () => {
    const { unmount } = render(<SettingsTab instance={forge} running onDeleted={() => {}} />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Install 47.4.26" })).toBeDisabled());
    expect(screen.getByText("Stop the server to change it")).toBeInTheDocument();
    unmount();

    render(<SettingsTab instance={makeInstance({ serverType: "vanilla" })} running={false} onDeleted={() => {}} />);
    expect(screen.queryByRole("region", { name: "Server software" })).not.toBeInTheDocument();
  });
});
