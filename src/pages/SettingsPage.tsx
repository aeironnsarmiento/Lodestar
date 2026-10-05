import { PageHeader } from "../components/PageHeader";
import { Switch } from "../components/glass/GlassInput";

interface SettingsPageProps {
  reduceEffects: boolean;
  onReduceEffects: (reduce: boolean) => void;
}

export function SettingsPage({ reduceEffects, onReduceEffects }: SettingsPageProps) {
  return (
    <div className="page">
      <PageHeader title="Settings" />
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
    </div>
  );
}
