/** True when running on macOS, where the tray is the menu bar and autostart is "at login". */
export function isMac(): boolean {
  return /Mac/i.test(navigator.platform || navigator.userAgent);
}

/** Wording for the OS-specific parts of the app. */
export function systemWords() {
  return isMac()
    ? { section: "System", tray: "menu bar", startAtLogin: "Open at login" }
    : { section: "Windows", tray: "tray", startAtLogin: "Start with Windows" };
}
