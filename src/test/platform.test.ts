import { afterEach, describe, expect, it, vi } from "vitest";
import { systemWords } from "../lib/platform";

describe("system wording", () => {
  afterEach(() => vi.restoreAllMocks());

  it("talks about the menu bar and login items on macOS", () => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
    expect(systemWords()).toEqual({ section: "System", tray: "menu bar", startAtLogin: "Open at login" });
  });

  it("keeps the Windows wording on Windows", () => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue("Win32");
    expect(systemWords()).toEqual({ section: "Windows", tray: "tray", startAtLogin: "Start with Windows" });
  });
});
