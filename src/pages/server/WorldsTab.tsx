import { useCallback, useEffect, useState } from "react";
import { GlassButton } from "../../components/glass/GlassButton";
import { Icon } from "../../components/Icon";
import { api, type Instance, type WorldInfo } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { errorMessage } from "../../state/store";

function formatCreated(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "—" : d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

export function WorldsTab({ instance }: { instance: Instance }) {
  const [worlds, setWorlds] = useState<WorldInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .listWorlds(instance.id)
      .then((w) => {
        setWorlds(w);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [instance.id]);

  // The current world changes on reset and switch; reload then.
  useEffect(load, [load, instance.currentWorld]);

  const play = async (name: string) => {
    setError(null);
    try {
      await api.switchWorld(instance.id, name);
      load();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const copySeed = async (w: WorldInfo) => {
    if (!w.seed) return;
    try {
      await navigator.clipboard.writeText(w.seed);
      setCopied(w.name);
      setTimeout(() => setCopied((c) => (c === w.name ? null : c)), 1500);
    } catch {
      // Clipboard refused; the seed is visible in the table.
    }
  };

  return (
    <section className="surface panel">
      <h2 className="section-title">Kept worlds</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        Each reset keeps the previous world; the 10 most recent are kept. Switching applies on the next start.
      </p>
      {error && <p className="error-text">{error}</p>}
      {worlds === null ? (
        <div className="empty">Loading…</div>
      ) : worlds.length === 0 ? (
        <div className="empty">No worlds yet. The first one is created when the server starts.</div>
      ) : (
        <table className="data" aria-label="Kept worlds">
          <thead>
            <tr>
              <th>World</th>
              <th>Seed</th>
              <th>Size</th>
              <th>Created</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {worlds.map((w) => (
              <tr key={w.name} className={w.current ? "current" : undefined}>
                <td>
                  <span className="mono">{w.name}</span>
                  {w.current && <span className="tag">Current</span>}
                </td>
                <td className="mono seed">{w.seed ?? "—"}</td>
                <td>{formatBytes(w.sizeBytes)}</td>
                <td>{formatCreated(w.createdAt)}</td>
                <td>
                  <span className="row" style={{ justifyContent: "flex-end", gap: 6 }}>
                    <GlassButton
                      size="sm"
                      icon={<Icon name={copied === w.name ? "check" : "copy"} size={13} />}
                      disabled={!w.seed}
                      onClick={() => copySeed(w)}
                      aria-label={`Copy seed of ${w.name}`}
                    >
                      {copied === w.name ? "Copied" : "Copy seed"}
                    </GlassButton>
                    <GlassButton
                      size="sm"
                      icon={<Icon name="play" size={11} />}
                      disabled={w.current}
                      onClick={() => play(w.name)}
                      aria-label={`Play ${w.name}`}
                    >
                      Play this world
                    </GlassButton>
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
