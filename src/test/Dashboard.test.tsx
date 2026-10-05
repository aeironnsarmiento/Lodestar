import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(() => Promise.resolve()) }));

import App from "../App";
import { Dashboard } from "../pages/Dashboard";
import { ServerCard } from "../components/ServerCard";
import type { ServerState } from "../lib/api";
import { resetStore, setState } from "../state/store";
import { makeInstance, makeSnapshot, settings, versions } from "./fixtures";

const noop = () => {};

describe("Dashboard", () => {
  beforeEach(() => {
    invoke.mockReset();
    resetStore();
  });

  it("deletes a stopped server from its card after confirming, but not a running one", async () => {
    const user = userEvent.setup();
    const stopped = makeInstance({ id: "old", name: "Old world" });
    const online = makeInstance({ id: "live", name: "Live", port: 25566 });
    let instances = [stopped, online];
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "delete_instance") {
        instances = [online];
        return Promise.resolve(null);
      }
      if (cmd === "list_instances") return Promise.resolve(instances);
      return Promise.resolve(null);
    });
    setState({ instances, runtime: { live: makeSnapshot({ id: "live", state: "online", port: 25566 }) } });
    render(<Dashboard onOpen={noop} />);

    expect(screen.getByRole("button", { name: "Delete Live" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Delete Old world" }));
    const dialog = screen.getByRole("dialog", { name: "Delete Old world?" });
    await user.click(within(dialog).getByRole("button", { name: /delete forever/i }));

    expect(invoke).toHaveBeenCalledWith("delete_instance", { id: "old" });
    await waitFor(() => expect(screen.queryByRole("article", { name: "Old world" })).not.toBeInTheDocument());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("creates a Fabric 26.3 server that shows setup progress, then Stopped when ready", async () => {
    const user = userEvent.setup();
    const created = makeInstance({ provision: { state: "pending" } });
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "list_versions") return Promise.resolve(versions);
      if (cmd === "create_instance") return Promise.resolve(created);
      if (cmd === "list_instances") return Promise.resolve([{ ...created, provision: { state: "running", message: "Downloading Java 25" } }]);
      return Promise.resolve(null);
    });
    render(<Dashboard onOpen={noop} />);

    await user.click(screen.getAllByRole("button", { name: /new server/i })[0]);
    await waitFor(() => expect(screen.getByLabelText("Minecraft version")).toHaveValue("26.3"));
    await user.click(screen.getByRole("button", { name: /create server/i }));

    const card = await screen.findByRole("article", { name: "Speedrun" });
    expect(within(card).getByText("Setting up")).toBeInTheDocument();
    expect(within(card).getByText("Downloading Java 25")).toBeInTheDocument();
    expect(within(card).getByRole("progressbar")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: /launch/i })).toBeDisabled();

    act(() => setState({ instances: [makeInstance()] }));
    expect(within(card).getByText("Stopped")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: /launch/i })).toBeEnabled();
  });

  it("shows a provisioning failure with a retry action and no launch", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue(undefined);
    act(() =>
      setState({ instances: [makeInstance({ launch: null, provision: { state: "failed", message: "Mojang has no server download for 26.3." } })] }),
    );
    render(<Dashboard onOpen={noop} />);
    const card = screen.getByRole("article", { name: "Speedrun" });
    expect(within(card).getByText("Setup failed")).toBeInTheDocument();
    expect(within(card).getByText(/no server download/)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: /launch/i })).not.toBeInTheDocument();
    await user.click(within(card).getByRole("button", { name: /retry setup/i }));
    expect(invoke).toHaveBeenCalledWith("retry_provision", { id: "speedrun" });
  });

  it("declining the EULA cancels the launch", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "accept_eula") return Promise.resolve({ ...settings, eulaAcceptedAt: "2026-10-05T10:00:00Z" });
      return Promise.resolve(undefined);
    });
    act(() => setState({ instances: [makeInstance()], settings }));
    render(<App />);

    const card = screen.getByRole("article", { name: "Speedrun" });
    await user.click(within(card).getByRole("button", { name: /launch/i }));
    const dialog = screen.getByRole("dialog", { name: /minecraft eula/i });
    expect(within(dialog).getByRole("link", { name: /read the minecraft eula/i })).toHaveAttribute("href", "https://aka.ms/MinecraftEULA");
    await user.click(within(dialog).getByRole("button", { name: /decline/i }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("start_server", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("accept_eula");

    // Accepting records it app-wide, then launches.
    await user.click(within(card).getByRole("button", { name: /launch/i }));
    await user.click(screen.getByRole("button", { name: /i agree/i }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("start_server", { id: "speedrun" }));
    expect(invoke).toHaveBeenCalledWith("accept_eula");

    // Next launch does not ask again.
    invoke.mockClear();
    await user.click(within(card).getByRole("button", { name: /launch/i }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("start_server", { id: "speedrun" });
  });
});

describe("Server card buttons", () => {
  const cases: [ServerState, { launch: boolean; stop: boolean; reset: boolean }][] = [
    ["stopped", { launch: true, stop: false, reset: true }],
    ["preparing", { launch: false, stop: false, reset: false }],
    ["starting", { launch: false, stop: true, reset: false }],
    ["online", { launch: false, stop: true, reset: true }],
    ["stopping", { launch: false, stop: false, reset: false }],
  ];

  it.each(cases)("reflect the %s state", (state, expected) => {
    render(
      <ServerCard
        instance={makeInstance()}
        snap={makeSnapshot({ state })}
        onOpen={noop}
        onLaunch={noop}
        onStop={noop}
        onReset={noop}
        onRetry={noop}
      />,
    );
    const launch = screen.queryByRole("button", { name: /launch/i });
    const stop = screen.queryByRole("button", { name: /^stop$/i });
    const reset = screen.getByRole("button", { name: /^reset$/i });
    expect(Boolean(launch && !(launch as HTMLButtonElement).disabled)).toBe(expected.launch);
    expect(Boolean(stop && !(stop as HTMLButtonElement).disabled)).toBe(expected.stop);
    expect(!(reset as HTMLButtonElement).disabled).toBe(expected.reset);
  });

  it("offers restart while online and force kill while stopping", async () => {
    const user = userEvent.setup();
    const onRestart = vi.fn();
    const onKill = vi.fn();
    const props = { instance: makeInstance(), onOpen: noop, onLaunch: noop, onStop: noop, onRestart, onKill, onRetry: noop };
    const { rerender } = render(<ServerCard {...props} snap={makeSnapshot({ state: "online" })} />);
    await user.click(screen.getByRole("button", { name: "Restart" }));
    expect(onRestart).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /force kill/i })).not.toBeInTheDocument();

    rerender(<ServerCard {...props} snap={makeSnapshot({ state: "stopping" })} />);
    expect(screen.queryByRole("button", { name: "Restart" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /force kill/i }));
    expect(onKill).toHaveBeenCalled();
  });

  it("shows live CPU, RAM and players while online", () => {
    render(
      <ServerCard
        instance={makeInstance()}
        snap={makeSnapshot({ state: "online", cpuPercent: 12.4, memoryBytes: 2_147_483_648, players: ["Steve", "Alex"] })}
        onOpen={noop}
        onLaunch={noop}
        onStop={noop}
        onRetry={noop}
      />,
    );
    expect(screen.getByText("12%")).toBeInTheDocument();
    expect(screen.getByText("2.0 GB")).toBeInTheDocument();
    expect(screen.getByText("2/10")).toBeInTheDocument();
    expect(screen.getByText("Online")).toBeInTheDocument();
  });
});
