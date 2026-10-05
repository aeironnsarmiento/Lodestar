import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PageHeader } from "../components/PageHeader";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassInput, Switch } from "../components/glass/GlassInput";
import { Icon } from "../components/Icon";
import { api } from "../lib/api";
import { saveSettings, useStore } from "../state/store";
import {
  applyGlassOpacity,
  applyWallpaper,
  GLASS_OPACITY_MAX,
  GLASS_OPACITY_MIN,
  readStoredGlassOpacity,
  readStoredWallpaper,
  WALLPAPERS,
  type Wallpaper,
} from "../lib/theme";
import aurora from "../assets/backgrounds/aurora.svg";
import orchid from "../assets/backgrounds/orchid.svg";
import dune from "../assets/backgrounds/dune.svg";
import graphite from "../assets/backgrounds/graphite.svg";
import frost from "../assets/backgrounds/frost.svg";

const WALLPAPER_ART: Record<Exclude<Wallpaper, "auto">, string> = { aurora, orchid, dune, graphite, frost };

interface SettingsPageProps {
  reduceEffects: boolean;
  onReduceEffects: (reduce: boolean) => void;
}

export function SettingsPage({ reduceEffects, onReduceEffects }: SettingsPageProps) {
  const settings = useStore((s) => s.settings);
  const error = useStore((s) => s.error);

  return (
    <div className="page">
      <PageHeader title="Settings" />
      <div className="stack" style={{ gap: 16 }}>
        <AppearanceSection reduceEffects={reduceEffects} onReduceEffects={onReduceEffects} />

        <section className="surface panel">
          <h2 className="section-title">Windows</h2>
          <div className="setting-row">
            <div>
              <div className="label">Close to tray</div>
              <div className="hint">Closing the window keeps Lodestar and your servers running in the tray.</div>
            </div>
            <Switch
              label="Close to tray"
              checked={settings?.closeToTray ?? true}
              disabled={!settings}
              onChange={(v) => saveSettings({ closeToTray: v })}
            />
          </div>
          <div className="setting-row">
            <div>
              <div className="label">Start with Windows</div>
              <div className="hint">Opens minimized in the tray and starts servers set to start with the app.</div>
            </div>
            <Switch
              label="Start with Windows"
              checked={settings?.startWithWindows ?? false}
              disabled={!settings}
              onChange={(v) => saveSettings({ startWithWindows: v })}
            />
          </div>
          <div className="setting-row">
            <div>
              <div className="label">Quit Lodestar</div>
              <div className="hint">Stops every server cleanly, then closes the app.</div>
            </div>
            <GlassButton size="sm" variant="danger" icon={<Icon name="kill" size={13} />} onClick={() => api.quitApp().catch(() => {})}>
              Quit
            </GlassButton>
          </div>
        </section>

        <CurseForgeSection />

        <section className="surface panel">
          <h2 className="section-title">Minecraft EULA</h2>
          <p className="muted" style={{ margin: 0 }}>
            {settings?.eulaAcceptedAt
              ? `Accepted on ${new Date(settings.eulaAcceptedAt).toLocaleDateString()}.`
              : "Not accepted yet. You will be asked the first time you launch a server."}
          </p>
        </section>
        {error && <p className="error-text">{error}</p>}
      </div>
    </div>
  );
}

interface AppearanceSectionProps {
  reduceEffects: boolean;
  onReduceEffects: (reduce: boolean) => void;
}

/** Wallpaper, glass opacity and effects. Remembered on this PC. */
function AppearanceSection({ reduceEffects, onReduceEffects }: AppearanceSectionProps) {
  const [wallpaper, setWallpaper] = useState<Wallpaper>(readStoredWallpaper);
  const [opacity, setOpacity] = useState(readStoredGlassOpacity);

  const pick = (w: Wallpaper) => {
    applyWallpaper(w);
    setWallpaper(w);
  };

  return (
    <section className="surface panel">
      <h2 className="section-title">Appearance</h2>
      <div className="setting-row" style={{ display: "block" }}>
        <div className="label">Wallpaper</div>
        <div className="wallpapers" role="group" aria-label="Wallpaper">
          {WALLPAPERS.map((w) => (
            <button
              key={w}
              type="button"
              className="wallpaper-tile"
              aria-pressed={wallpaper === w}
              style={{ backgroundImage: `url("${w === "auto" ? aurora : WALLPAPER_ART[w]}")` }}
              onClick={() => pick(w)}
            >
              {w === "auto" && <span className="split" style={{ backgroundImage: `url("${frost}")` }} />}
              <span className="wallpaper-label">{w === "auto" ? "Match theme" : w[0].toUpperCase() + w.slice(1)}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="setting-row">
        <div>
          <div className="label">Glass opacity</div>
          <div className="hint">Higher is more solid; lower lets more of the wallpaper through.</div>
        </div>
        <div className="slider-row setting-control">
          <input
            type="range"
            aria-label="Glass opacity"
            min={GLASS_OPACITY_MIN}
            max={GLASS_OPACITY_MAX}
            step={0.01}
            value={opacity}
            onChange={(e) => {
              const v = Number(e.target.value);
              applyGlassOpacity(v);
              setOpacity(v);
            }}
          />
          <span className="slider-value">{Math.round(opacity * 100)}%</span>
        </div>
      </div>
      <div className="setting-row">
        <div>
          <div className="label">Reduce effects</div>
          <div className="hint">Turns off the frosted blur and animations for a flat, lighter look.</div>
        </div>
        <Switch label="Reduce effects" checked={reduceEffects} onChange={onReduceEffects} />
      </div>
    </section>
  );
}

const CURSEFORGE_CONSOLE = "https://console.curseforge.com/";

/** The user's own CurseForge API key; CurseForge does not let apps ship one. */
function CurseForgeSection() {
  const saved = useStore((s) => s.settings?.curseforgeApiKey ?? "");
  const ready = useStore((s) => s.settings !== null);
  const [key, setKey] = useState(saved);
  const [shown, setShown] = useState(false);
  useEffect(() => setKey(saved), [saved]);

  const commit = () => {
    if (key.trim() !== saved) saveSettings({ curseforgeApiKey: key.trim() });
  };

  return (
    <section className="surface panel">
      <h2 className="section-title">CurseForge</h2>
      <div className="setting-row">
        <div>
          <div className="label">API key</div>
          <div className="hint">
            Needed to browse and install from CurseForge. Get a free key from the{" "}
            <a
              href={CURSEFORGE_CONSOLE}
              onClick={(e) => {
                e.preventDefault();
                openUrl(CURSEFORGE_CONSOLE).catch(() => {});
              }}
            >
              CurseForge console
            </a>
            . Modrinth works without one.
          </div>
        </div>
        <div className="setting-control row">
          <GlassInput
            aria-label="CurseForge API key"
            type={shown ? "text" : "password"}
            placeholder="Paste your key"
            value={key}
            disabled={!ready}
            onChange={(e) => setKey(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => e.key === "Enter" && commit()}
          />
          <GlassButton size="sm" iconOnly variant="ghost" aria-label={shown ? "Hide key" : "Show key"} icon={<Icon name="search" size={14} />} onClick={() => setShown((v) => !v)} />
        </div>
      </div>
    </section>
  );
}
