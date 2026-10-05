import { useCallback, useEffect, useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { GlassButton } from "../components/glass/GlassButton";
import { Icon } from "../components/Icon";
import { api, type JavaRuntime } from "../lib/api";
import { formatBytes } from "../lib/format";
import { errorMessage } from "../state/store";

export function JavaPage() {
  const [runtimes, setRuntimes] = useState<JavaRuntime[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .listJavaRuntimes()
      .then((r) => {
        setRuntimes(r);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(load, [load]);

  const remove = async (major: number) => {
    try {
      await api.removeJavaRuntime(major);
      load();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="page">
      <PageHeader title="Java runtimes" subtitle="Downloaded automatically for each server; nothing is installed system-wide" />
      {error && <p className="error-text">{error}</p>}
      <section className="surface panel">
        {runtimes === null ? (
          <div className="empty">Loading…</div>
        ) : runtimes.length === 0 ? (
          <div className="empty">
            <Icon name="cup" size={28} />
            No runtimes yet. The right Java downloads the first time a server needs it.
          </div>
        ) : (
          <table className="data" aria-label="Installed Java runtimes">
            <thead>
              <tr>
                <th>Runtime</th>
                <th>Size</th>
                <th>Location</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {runtimes.map((r) => (
                <tr key={r.major}>
                  <td>
                    <strong>Java {r.major}</strong>
                    {r.inUse && <span className="faint"> · in use</span>}
                  </td>
                  <td>{formatBytes(r.sizeBytes)}</td>
                  <td className="mono faint" style={{ fontSize: 12 }}>
                    {r.path}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    <GlassButton
                      size="sm"
                      variant="danger"
                      icon={<Icon name="trash" size={14} />}
                      disabled={r.inUse}
                      title={r.inUse ? "A running server uses this runtime" : undefined}
                      onClick={() => remove(r.major)}
                    >
                      Remove
                    </GlassButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
