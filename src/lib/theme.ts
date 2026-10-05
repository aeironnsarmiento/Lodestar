export type Theme = "light" | "dark";

const THEME_KEY = "glasscraft.theme";
const EFFECTS_KEY = "glasscraft.reduceEffects";

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/** The theme remembered on this machine; dark by default. */
export function readStoredTheme(): Theme {
  const t = storage()?.getItem(THEME_KEY);
  return t === "light" ? "light" : "dark";
}

/**
 * Applies a theme to the document root and remembers it locally. The backend
 * settings file is the source of truth once loaded; localStorage only exists so the
 * first paint (before the backend answers) uses the right theme.
 */
export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  storage()?.setItem(THEME_KEY, theme);
}

export function readStoredReduceEffects(): boolean {
  return storage()?.getItem(EFFECTS_KEY) === "1";
}

export function applyReduceEffects(reduce: boolean): void {
  if (reduce) document.documentElement.dataset.effects = "reduced";
  else delete document.documentElement.dataset.effects;
  storage()?.setItem(EFFECTS_KEY, reduce ? "1" : "0");
}

/**
 * SVG refraction needs `backdrop-filter: url(...)`, which only Chromium (WebView2)
 * renders. Anything else falls back to plain blur.
 */
export function supportsRefraction(): boolean {
  if (typeof CSS === "undefined" || typeof CSS.supports !== "function") return false;
  if (!CSS.supports("backdrop-filter", "blur(1px)")) return false;
  return /\bChrome\/\d+/.test(navigator.userAgent);
}
