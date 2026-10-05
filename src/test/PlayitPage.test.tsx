import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(() => Promise.resolve()) }));

import { PlayitPage } from "../pages/PlayitPage";
import { resetStore, setState } from "../state/store";
import { makeInstance } from "./fixtures";

describe("playit.gg page", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue("https://playit.gg/claim/0a1b2c3d4e");
    resetStore();
  });

  it("starts setup when not linked", async () => {
    const user = userEvent.setup();
    act(() => setState({ playit: { state: "notSetUp", claimUrl: null, message: null, tunnels: [] } }));
    render(<PlayitPage />);
    expect(screen.getByText("Not set up")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /set up playit\.gg/i }));
    expect(invoke).toHaveBeenCalledWith("playit_setup");
  });

  it("shows the claim link while waiting for approval", () => {
    act(() =>
      setState({
        playit: { state: "waitingForClaim", claimUrl: "https://playit.gg/claim/0a1b2c3d4e", message: "Approve Glasscraft in your browser to finish.", tunnels: [] },
      }),
    );
    render(<PlayitPage />);
    expect(screen.getByText("Waiting for you to link")).toBeInTheDocument();
    expect(screen.getByText("https://playit.gg/claim/0a1b2c3d4e")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /open link again/i })).toBeInTheDocument();
  });

  it("lists tunnels with their servers, addresses and problems", () => {
    act(() =>
      setState({
        instances: [makeInstance({ name: "Speedrun", port: 25565 }), makeInstance({ id: "b", name: "Practice", port: 25566 })],
        playit: {
          state: "linked",
          claimUrl: null,
          message: null,
          tunnels: [
            { port: 25565, state: "connected", address: "glass-runs.gl.joinmc.link", message: null },
            { port: 25566, state: "limitReached", address: null, message: "Your playit.gg account has no free tunnel left." },
          ],
        },
      }),
    );
    render(<PlayitPage />);
    expect(screen.getByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("glass-runs.gl.joinmc.link")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /copy speedrun · port 25565 address/i })).toBeInTheDocument();
    expect(screen.getByText(/limit reached — your playit\.gg account has no free tunnel left/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /re-link account/i })).toBeInTheDocument();
  });
});
