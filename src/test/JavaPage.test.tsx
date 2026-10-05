import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { JavaPage } from "../pages/JavaPage";

const runtimes = [
  { major: 21, path: "C:\\Lodestar\\runtimes\\java\\jre-21", sizeBytes: 52_428_800, inUse: false },
  { major: 25, path: "C:\\Lodestar\\runtimes\\java\\jre-25", sizeBytes: 60_817_408, inUse: true },
];

describe("Java runtimes page", () => {
  beforeEach(() => invoke.mockReset());

  it("lists installed runtimes with sizes and disables remove for one in use", async () => {
    invoke.mockResolvedValueOnce(runtimes);
    render(<JavaPage />);
    const table = await screen.findByRole("table", { name: /installed java runtimes/i });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);

    expect(within(rows[0]).getByText("Java 21")).toBeInTheDocument();
    expect(within(rows[0]).getByText("50 MB")).toBeInTheDocument();
    expect(within(rows[0]).getByRole("button", { name: /remove/i })).toBeEnabled();

    expect(within(rows[1]).getByText("Java 25")).toBeInTheDocument();
    expect(within(rows[1]).getByText("58 MB")).toBeInTheDocument();
    expect(within(rows[1]).getByRole("button", { name: /remove/i })).toBeDisabled();
  });

  it("removes a runtime and reloads the list", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValueOnce(runtimes).mockResolvedValueOnce(undefined).mockResolvedValueOnce([runtimes[1]]);
    render(<JavaPage />);
    const table = await screen.findByRole("table");
    await user.click(within(within(table).getAllByRole("row")[1]).getByRole("button", { name: /remove/i }));
    expect(invoke).toHaveBeenCalledWith("remove_java_runtime", { major: 21 });
    expect(await screen.findAllByRole("row")).toHaveLength(2);
  });
});
