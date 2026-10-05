import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { NewServerDialog } from "../dialogs/NewServerDialog";
import { makeInstance, versions } from "./fixtures";

function optionIds(): string[] {
  const select = screen.getByLabelText("Minecraft version") as HTMLSelectElement;
  return Array.from(select.options).map((o) => o.value);
}

describe("New server dialog", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "list_versions") return Promise.resolve(versions);
      if (cmd === "create_instance") return Promise.resolve(makeInstance({ provision: { state: "pending" } }));
      return Promise.reject(`unexpected ${cmd}`);
    });
  });

  it("lists releases with dates and adds snapshots when the filter is on", async () => {
    const user = userEvent.setup();
    render(<NewServerDialog onClose={() => {}} onCreated={() => {}} />);
    await waitFor(() => expect(optionIds()).toContain("26.3"));
    expect(optionIds()).not.toContain("26.4-snapshot-2");
    const select = screen.getByLabelText("Minecraft version");
    expect(within(select).getByText(/26\.3 — Sep \d+, 2026/)).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: /show snapshots/i }));
    expect(optionIds()[0]).toBe("26.4-snapshot-2");
  });

  it("shows only Minecraft 1.17+ for Forge and NeoForge", async () => {
    const user = userEvent.setup();
    render(<NewServerDialog onClose={() => {}} onCreated={() => {}} />);
    await waitFor(() => expect(optionIds()).toContain("1.16.5"));

    await user.click(screen.getByRole("radio", { name: "Forge" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_versions", { serverType: "forge" }));
    await waitFor(() => expect(optionIds()).toContain("1.17.1"));
    expect(optionIds()).not.toContain("1.16.5");

    await user.click(screen.getByRole("radio", { name: "NeoForge" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_versions", { serverType: "neoforge" }));
    await waitFor(() => expect(optionIds()).toContain("26.3"));
    expect(optionIds()).not.toContain("1.16.5");
  });

  it("submits a Fabric 26.3 server with its world options", async () => {
    const user = userEvent.setup();
    const onCreated = vi.fn();
    render(<NewServerDialog onClose={() => {}} onCreated={onCreated} />);
    await waitFor(() => expect(optionIds()).toContain("26.3"));

    const name = screen.getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "Friday runs");
    await user.type(screen.getByLabelText("Seed"), "speedrun123");
    await user.selectOptions(screen.getByLabelText("Difficulty"), "hard");
    await user.clear(screen.getByLabelText("Player limit"));
    await user.type(screen.getByLabelText("Player limit"), "4");
    await user.click(screen.getByRole("button", { name: /create server/i }));

    expect(invoke).toHaveBeenCalledWith("create_instance", {
      new: {
        name: "Friday runs",
        serverType: "fabric",
        mcVersion: "26.3",
        seed: "speedrun123",
        gameMode: "survival",
        difficulty: "hard",
        maxPlayers: 4,
      },
    });
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
  });
});
