import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../App";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassPanel } from "../components/glass/GlassPanel";
import { GlassPill } from "../components/glass/GlassPill";
import { EffectsContext } from "../components/glass/effects";
import { supportsRefraction } from "../lib/theme";

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
});

describe("glass effects", () => {
  it("renders without blur and refraction classes when Reduce effects is on", () => {
    render(
      <EffectsContext.Provider value={{ reduceEffects: true, refraction: true }}>
        <GlassPanel data-testid="panel" />
        <GlassButton>Launch</GlassButton>
        <GlassPill data-testid="pill">Online</GlassPill>
      </EffectsContext.Provider>,
    );
    for (const el of [screen.getByTestId("panel"), screen.getByRole("button"), screen.getByTestId("pill")]) {
      expect(el).not.toHaveClass("glass-blur");
      expect(el).not.toHaveClass("glass-refract");
      expect(el).toHaveClass("glass-flat");
    }
  });

  it("applies refraction to controls only when the feature check passes", () => {
    const { rerender } = render(
      <EffectsContext.Provider value={{ reduceEffects: false, refraction: true }}>
        <GlassButton>Launch</GlassButton>
        <GlassPanel data-testid="panel" />
      </EffectsContext.Provider>,
    );
    expect(screen.getByRole("button")).toHaveClass("glass-refract", "glass-blur");
    // Large chrome never refracts.
    expect(screen.getByTestId("panel")).not.toHaveClass("glass-refract");

    rerender(
      <EffectsContext.Provider value={{ reduceEffects: false, refraction: false }}>
        <GlassButton>Launch</GlassButton>
        <GlassPanel data-testid="panel" />
      </EffectsContext.Provider>,
    );
    expect(screen.getByRole("button")).not.toHaveClass("glass-refract");
    expect(screen.getByRole("button")).toHaveClass("glass-blur");
  });

  it("fails the refraction feature check outside Chromium", () => {
    expect(supportsRefraction()).toBe(false);
    vi.stubGlobal("CSS", { supports: () => true });
    const ua = vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 Firefox/140.0");
    expect(supportsRefraction()).toBe(false);
    ua.mockReturnValue("Mozilla/5.0 AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0");
    expect(supportsRefraction()).toBe(true);
    vi.stubGlobal("CSS", { supports: () => false });
    expect(supportsRefraction()).toBe(false);
    vi.unstubAllGlobals();
    ua.mockRestore();
  });
});
