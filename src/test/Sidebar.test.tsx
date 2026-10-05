import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";

describe("Sidebar navigation", () => {
  it("navigates between app areas and highlights the active item", async () => {
    const user = userEvent.setup();
    render(<App />);
    const nav = screen.getByRole("complementary", { name: /main navigation/i });

    const dashboard = screen.getByRole("button", { name: "Dashboard" });
    expect(dashboard).toHaveClass("active");
    expect(nav).toBeInTheDocument();

    for (const [label, heading] of [
      ["playit.gg", "playit.gg"],
      ["Java runtimes", "Java runtimes"],
      ["Settings", "Settings"],
      ["Dashboard", "Dashboard"],
    ] as const) {
      await user.click(screen.getByRole("button", { name: label }));
      expect(screen.getByRole("heading", { level: 1, name: heading })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: label })).toHaveClass("active");
      expect(screen.getByRole("button", { name: label })).toHaveAttribute("aria-current", "page");
    }
    expect(screen.getByRole("button", { name: "Settings" })).not.toHaveClass("active");
  });
});
