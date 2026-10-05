import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { SettingsTab } from "../pages/server/SettingsTab";
import { makeInstance } from "./fixtures";

describe("Scheduled restart settings", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string, args: { instance?: unknown }) =>
      cmd === "update_instance" ? Promise.resolve(args.instance) : Promise.resolve(undefined),
    );
  });

  it("adds and removes daily restart times, switches modes, and saves them to the instance", async () => {
    const user = userEvent.setup();
    render(<SettingsTab instance={makeInstance({ restart: { times: ["04:00"], mode: "warn" } })} running={false} onDeleted={() => {}} />);
    const times = screen.getByLabelText("Restart times");
    expect(within(times).getByText("04:00")).toBeInTheDocument();

    const input = screen.getByLabelText("New restart time");
    await user.clear(input);
    await user.type(input, "16:30");
    await user.click(screen.getByRole("button", { name: /add time/i }));
    expect(within(times).getByText("16:30")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Remove 04:00" }));
    expect(within(times).queryByText("04:00")).not.toBeInTheDocument();

    const modes = screen.getByRole("radiogroup", { name: /when players are online/i });
    expect(within(modes).getByRole("radio", { name: /warn, then restart/i })).toBeChecked();
    await user.click(within(modes).getByRole("radio", { name: /wait until empty/i }));

    await user.click(screen.getByRole("button", { name: /save changes/i }));
    expect(invoke).toHaveBeenCalledWith("update_instance", {
      instance: expect.objectContaining({ restart: { times: ["16:30"], mode: "postpone" } }),
    });
    await waitFor(() => expect(screen.getByText(/saved/i)).toBeInTheDocument());
  });

  it("ignores duplicate times", async () => {
    const user = userEvent.setup();
    render(<SettingsTab instance={makeInstance({ restart: { times: ["04:00"], mode: "warn" } })} running={false} onDeleted={() => {}} />);
    await user.click(screen.getByRole("button", { name: /add time/i }));
    expect(within(screen.getByLabelText("Restart times")).getAllByText("04:00")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /save changes/i })).toBeDisabled();
  });
});
