import { PageHeader } from "../components/PageHeader";
import { GlassButton } from "../components/glass/GlassButton";
import { Switch } from "../components/glass/GlassInput";
import { Icon } from "../components/Icon";
import { api } from "../lib/api";
import { saveSettings, useStore } from "../state/store";

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
        <section className="surface panel">
          <h2 className="section-title">Appearance</h2>
          <div className="setting-row">
            <div>
              <div className="label">Reduce effects</div>
              <div className="hint">Turns off blur and refraction for a flat, lighter look.</div>
            </div>
            <Switch label="Reduce effects" checked={reduceEffects} onChange={onReduceEffects} />
          </div>
        </section>

        <section className="surface panel">
          <h2 className="section-title">Windows</h2>
          <div className="setting-row">
            <div>
              <div className="label">Close to tray</div>
              <div className="hint">Closing the window keeps Glasscraft and your servers running in the tray.</div>
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
              <div className="label">Quit Glasscraft</div>
              <div className="hint">Stops every server cleanly, then closes the app.</div>
            </div>
            <GlassButton size="sm" variant="danger" icon={<Icon name="kill" size={13} />} onClick={() => api.quitApp().catch(() => {})}>
              Quit
            </GlassButton>
          </div>
        </section>

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
