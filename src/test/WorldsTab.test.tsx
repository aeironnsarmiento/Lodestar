import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { WorldsTab } from "../pages/server/WorldsTab";
import { ServerCard } from "../components/ServerCard";
import { makeInstance, makeSnapshot } from "./fixtures";

const worlds = [
  { name: "run_2026-10-05_21-30-05", seed: "-4172144997902289642", createdAt: "2026-10-05T21:30:05+02:00", sizeBytes: 12_582_912, current: true },
  { name: "run_2026-10-05_21-12-40", seed: "speedrun123", createdAt: "2026-10-05T21:12:40+02:00", sizeBytes: 9_437_184, current: false },
];

describe("Worlds tab", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "list_worlds") return Promise.resolve(worlds);
      return Promise.resolve(undefined);
    });
  });

  it("lists kept worlds with seeds, sizes and the current one", async () => {
    render(<WorldsTab instance={makeInstance({ currentWorld: worlds[0].name })} />);
    const table = await screen.findByRole("table", { name: /kept worlds/i });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);
    expect(within(rows[0]).getByText("Current")).toBeInTheDocument();
    expect(within(rows[0]).getByText("-4172144997902289642")).toBeInTheDocument();
    expect(within(rows[0]).getByText("12 MB")).toBeInTheDocument();
    expect(within(rows[0]).getByRole("button", { name: /play run_2026-10-05_21-30-05/i })).toBeDisabled();
    expect(within(rows[1]).getByText("speedrun123")).toBeInTheDocument();
  });

  it("switches to an older world and copies a seed", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<WorldsTab instance={makeInstance({ currentWorld: worlds[0].name })} />);
    const table = await screen.findByRole("table");
    const older = within(table).getAllByRole("row")[2];

    await user.click(within(older).getByRole("button", { name: /play run_2026-10-05_21-12-40/i }));
    expect(invoke).toHaveBeenCalledWith("switch_world", { id: "speedrun", name: "run_2026-10-05_21-12-40" });

    await user.click(within(older).getByRole("button", { name: /copy seed/i }));
    expect(writeText).toHaveBeenCalledWith("speedrun123");
    await waitFor(() => expect(within(older).getByText("Copied")).toBeInTheDocument());
  });
});

describe("Reset World button", () => {
  it("resets in one click with a random seed, or with a seed from the popover", async () => {
    const user = userEvent.setup();
    const onReset = vi.fn();
    const onOpen = vi.fn();
    render(
      <ServerCard
        instance={makeInstance()}
        snap={makeSnapshot({ state: "online" })}
        onOpen={onOpen}
        onLaunch={() => {}}
        onStop={() => {}}
        onReset={onReset}
        onRetry={() => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: /^reset$/i }));
    expect(onReset).toHaveBeenLastCalledWith(null, false);

    await user.click(screen.getByRole("button", { name: /new world options/i }));
    let pop = screen.getByRole("dialog", { name: /new world options/i });
    await user.type(within(pop).getByPlaceholderText("Random"), "speedrun123{Enter}");
    expect(onReset).toHaveBeenLastCalledWith("speedrun123", false);

    // The next world can be made hardcore from the same popover.
    await user.click(screen.getByRole("button", { name: /new world options/i }));
    pop = screen.getByRole("dialog", { name: /new world options/i });
    await user.click(within(pop).getByRole("switch", { name: "Hardcore" }));
    await user.click(within(pop).getByRole("button", { name: /reset world/i }));
    expect(onReset).toHaveBeenLastCalledWith(null, true);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();
  });
});
