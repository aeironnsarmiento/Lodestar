import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PageHeader } from "../components/PageHeader";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassPill } from "../components/glass/GlassPill";
import { Icon } from "../components/Icon";
import { api, type LinkState, type TunnelState } from "../lib/api";
import { AddressRow } from "./server/OverviewTab";
import { errorMessage, useStore } from "../state/store";

const STATE_LABEL: Record<LinkState, string> = {
  notSetUp: "Not set up",
  installing: "Installing agent",
  waitingForClaim: "Waiting for you to link",
  agentOffline: "Agent offline",
  linked: "Connected",
};

export const TUNNEL_LABEL: Record<TunnelState, string> = {
  pending: "Tunnel pending",
  connected: "Connected",
  limitReached: "Limit reached",
  error: "Tunnel error",
};

export function PlayitPage() {
  const playit = useStore((s) => s.playit);
  const instances = useStore((s) => s.instances);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const state: LinkState = playit?.state ?? "notSetUp";

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const linked = state === "linked" || state === "agentOffline";

  return (
    <div className="page">
      <PageHeader
        title="playit.gg"
        subtitle="Friends join from anywhere, no router setup"
        actions={
          <GlassPill className={`status status-playit-${state}`} data-state={state}>
            <span className="dot" aria-hidden="true" />
            {STATE_LABEL[state]}
          </GlassPill>
        }
      />
      <div className="stack" style={{ gap: 16 }}>
        <section className="surface panel">
          {state === "notSetUp" && (
            <div className="stack">
              <p style={{ margin: 0 }}>
                Lodestar runs its own playit.gg agent and gives each server a public address. Link a free playit.gg
                account once; the browser opens to approve it.
              </p>
              {playit?.message && <p className="error-text">{playit.message}</p>}
              <div>
                <GlassButton variant="primary" icon={<Icon name="link" size={15} />} disabled={busy} onClick={() => run(api.playitSetup)}>
                  Set up playit.gg
                </GlassButton>
              </div>
            </div>
          )}
          {state === "installing" && <p className="muted">Downloading and verifying the playit.gg agent…</p>}
          {state === "waitingForClaim" && (
            <div className="stack">
              <p style={{ margin: 0 }}>{playit?.message ?? "Approve Lodestar on playit.gg to finish linking."}</p>
              {playit?.claimUrl && <p className="mono address">{playit.claimUrl}</p>}
              <div className="row">
                <GlassButton
                  variant="primary"
                  icon={<Icon name="external" size={14} />}
                  onClick={() => playit?.claimUrl && openUrl(playit.claimUrl).catch(() => {})}
                >
                  Open link again
                </GlassButton>
                <GlassButton onClick={() => run(api.playitCancel)}>Cancel</GlassButton>
              </div>
            </div>
          )}
          {linked && (
            <div className="stack">
              <p style={{ margin: 0 }}>
                {state === "linked"
                  ? "Your playit.gg account is linked. Each server gets a public address the first time it launches."
                  : (playit?.message ?? "The agent is not running. It restarts on its own.")}
              </p>
              <div>
                <GlassButton size="sm" icon={<Icon name="link" size={14} />} disabled={busy} onClick={() => run(api.playitRelink)}>
                  Re-link account
                </GlassButton>
              </div>
            </div>
          )}
          {error && <p className="error-text">{error}</p>}
        </section>

        {linked && (
          <section className="surface panel">
            <h2 className="section-title">Tunnels</h2>
            {playit?.tunnels.length ? (
              playit.tunnels.map((t) => {
                const users = instances.filter((i) => i.port === t.port).map((i) => i.name);
                const label = `${users.join(", ") || "No server"} · port ${t.port}`;
                return t.state === "connected" ? (
                  <AddressRow key={t.port} label={label} value={t.address} />
                ) : (
                  <div key={t.port} className="setting-row">
                    <div>
                      <div className="hint">{label}</div>
                      <div className={t.state === "pending" ? "muted" : "error-text"}>
                        {TUNNEL_LABEL[t.state]}
                        {t.message ? ` — ${t.message}` : ""}
                      </div>
                    </div>
                    {t.state !== "pending" && (
                      <GlassButton size="sm" icon={<Icon name="restart" size={14} />} onClick={() => run(() => api.playitRetryTunnel(t.port))}>
                        Retry
                      </GlassButton>
                    )}
                  </div>
                );
              })
            ) : (
              <p className="muted" style={{ margin: 0 }}>
                No tunnels yet. Launch a server to create one for its port.
              </p>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
