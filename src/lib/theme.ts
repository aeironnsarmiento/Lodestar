export type Theme = "light" | "dark";

const THEME_KEY = "lodestar.theme";
const EFFECTS_KEY = "lodestar.reduceEffects";
const WALLPAPER_KEY = "lodestar.wallpaper";
const OPACITY_KEY = "lodestar.glassOpacity";

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

/** Frost by default; "auto" follows the theme (Aurora when dark, Frost when light). */
export const WALLPAPERS = ["frost", "auto", "aurora", "orchid", "dune", "graphite"] as const;
export type Wallpaper = (typeof WALLPAPERS)[number];

export function readStoredWallpaper(): Wallpaper {
  const w = storage()?.getItem(WALLPAPER_KEY);
  return (WALLPAPERS as readonly string[]).includes(w ?? "") ? (w as Wallpaper) : "frost";
}

export function applyWallpaper(wallpaper: Wallpaper): void {
  document.documentElement.dataset.wallpaper = wallpaper;
  storage()?.setItem(WALLPAPER_KEY, wallpaper);
}

export const GLASS_OPACITY_MIN = 0.2;
export const GLASS_OPACITY_MAX = 0.85;
export const GLASS_OPACITY_DEFAULT = 0.55;

export function readStoredGlassOpacity(): number {
  const n = Number(storage()?.getItem(OPACITY_KEY));
  return Number.isFinite(n) && n >= GLASS_OPACITY_MIN && n <= GLASS_OPACITY_MAX ? n : GLASS_OPACITY_DEFAULT;
}

/** How see-through the frame and its panes are. */
export function applyGlassOpacity(opacity: number): void {
  const value = Math.min(GLASS_OPACITY_MAX, Math.max(GLASS_OPACITY_MIN, opacity));
  const root = document.documentElement.style;
  root.setProperty("--glass-alpha-frame", value.toFixed(2));
  root.setProperty("--glass-alpha-pane", value.toFixed(2));
  storage()?.setItem(OPACITY_KEY, value.toFixed(2));
}
