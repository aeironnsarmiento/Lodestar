import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { GlassPanel } from "../components/glass/GlassPanel";
import { EffectsContext } from "../components/glass/effects";
import { applyGlassOpacity, applyWallpaper, readStoredGlassOpacity, readStoredWallpaper } from "../lib/theme";

describe("theme", () => {
  it("toggles the root theme attribute and keeps the choice after a reload", async () => {
    const user = userEvent.setup();
    const first = render(<App />);
    expect(document.documentElement.dataset.theme).toBe("dark");

    await user.click(screen.getByRole("button", { name: /switch to light mode/i }));
    expect(document.documentElement.dataset.theme).toBe("light");

    // "Reload": tear the app down, wipe the attribute, and mount again.
    first.unmount();
    delete document.documentElement.dataset.theme;
    render(<App />);
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(screen.getByRole("button", { name: /switch to dark mode/i })).toBeInTheDocument();
  });

  it("defaults to Frost and remembers the wallpaper and glass opacity", () => {
    expect(readStoredWallpaper()).toBe("frost");
    applyWallpaper("orchid");
    expect(document.documentElement.dataset.wallpaper).toBe("orchid");
    expect(readStoredWallpaper()).toBe("orchid");
    applyWallpaper("auto");
    expect(document.documentElement.dataset.wallpaper).toBe("auto");

    applyGlassOpacity(2);
    expect(readStoredGlassOpacity()).toBe(0.85);
    expect(document.documentElement.style.getPropertyValue("--glass-alpha-pane")).toBe("0.85");
  });
});

describe("glass blur budget", () => {
  it("blurs only the outer surface; nested surfaces are flat panes", () => {
    render(
      <GlassPanel data-testid="frame">
        <GlassPanel data-testid="pane">
          <GlassPanel layer data-testid="popover" />
        </GlassPanel>
      </GlassPanel>,
    );
    expect(screen.getByTestId("frame")).toHaveAttribute("data-blur", "on");
    expect(screen.getByTestId("pane")).toHaveAttribute("data-blur", "off");
    // A new layer (dialog, popover) blurs again even when nested.
    expect(screen.getByTestId("popover")).toHaveAttribute("data-blur", "on");
  });

  it("turns every blur off when Reduce effects is on", () => {
    render(
      <EffectsContext.Provider value={{ reduceEffects: true }}>
        <GlassPanel data-testid="frame">
          <GlassPanel layer data-testid="popover" />
        </GlassPanel>
      </EffectsContext.Provider>,
    );
    expect(screen.getByTestId("frame")).toHaveAttribute("data-blur", "off");
    expect(screen.getByTestId("popover")).toHaveAttribute("data-blur", "off");
  });
});
