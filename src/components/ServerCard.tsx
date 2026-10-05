import type { MouseEvent } from "react";
import { SERVER_TYPE_LABELS, type Instance, type Snapshot, type TaskProgress } from "../lib/api";
import { formatBytes, formatPercent } from "../lib/format";
import { GlassButton } from "./glass/GlassButton";
import { Icon } from "./Icon";
import { StatusBadge, type BadgeState } from "./StatusBadge";

interface ServerCardProps {
  instance: Instance;
  snap: Snapshot;
  progress?: TaskProgress;
  error?: string;
  onOpen: () => void;
  onLaunch: () => void;
  onStop: () => void;
  onReset?: () => void;
  onRetry: () => void;
}

export function badgeState(instance: Instance, snap: Snapshot): BadgeState {
  if (instance.provision.state === "pending" || instance.provision.state === "running") return "setup";
  if (instance.provision.state === "failed") return "setupFailed";
  return snap.state;
}

/** Stops the click from also opening the server page. */
const stop = (fn: () => void) => (e: MouseEvent) => {
  e.stopPropagation();
  fn();
};

export function ServerCard({ instance, snap, progress, error, onOpen, onLaunch, onStop, onReset, onRetry }: ServerCardProps) {
  const badge = badgeState(instance, snap);
  const ready = instance.provision.state === "ready";
  const s = snap.state;
  const running = s === "starting" || s === "online" || s === "stopping";
  const canLaunch = ready && (s === "stopped" || s === "crashed");
  const canStop = s === "starting" || s === "online";
  const canReset = ready && (s === "stopped" || s === "online" || s === "crashed");
  const pct = progress?.total ? Math.round((progress.done / progress.total) * 100) : null;

  return (
    <article
      className="surface server-card"
      aria-label={instance.name}
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => e.key === "Enter" && e.target === e.currentTarget && onOpen()}
    >
      <header className="server-card-head">
        <div className="server-card-title">
          <h3>{instance.name}</h3>
          <span className="faint">
            {SERVER_TYPE_LABELS[instance.serverType]} {instance.mcVersion}
          </span>
        </div>
        <StatusBadge state={badge} />
      </header>

      {instance.provision.state === "running" || instance.provision.state === "pending" ? (
        <div className="provision">
          <div className="muted">{instance.provision.state === "running" ? instance.provision.message : "Waiting…"}</div>
          <div className="progress" role="progressbar" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
            <div className={pct === null ? "bar indeterminate" : "bar"} style={pct === null ? undefined : { width: `${pct}%` }} />
          </div>
        </div>
      ) : instance.provision.state === "failed" ? (
        <div className="provision">
          <div className="error-text">{instance.provision.message}</div>
        </div>
      ) : (
        <dl className="server-card-stats">
          <div>
            <dt>CPU</dt>
            <dd>{running ? formatPercent(snap.cpuPercent) : "—"}</dd>
          </div>
          <div>
            <dt>RAM</dt>
            <dd>{running && snap.memoryBytes ? formatBytes(snap.memoryBytes) : "—"}</dd>
          </div>
          <div>
            <dt>Players</dt>
            <dd>
              {snap.players.length}/{instance.maxPlayers}
            </dd>
          </div>
        </dl>
      )}

      {error && <div className="error-text card-error">{error}</div>}

      <footer className="server-card-actions">
        {instance.provision.state === "failed" ? (
          <GlassButton size="sm" icon={<Icon name="reset" size={14} />} onClick={stop(onRetry)}>
            Retry setup
          </GlassButton>
        ) : (
          <>
            {canStop ? (
              <GlassButton size="sm" icon={<Icon name="stop" size={12} />} onClick={stop(onStop)}>
                Stop
              </GlassButton>
            ) : (
              <GlassButton size="sm" variant="primary" icon={<Icon name="play" size={12} />} disabled={!canLaunch} onClick={stop(onLaunch)}>
                Launch
              </GlassButton>
            )}
            {onReset && (
              <GlassButton size="sm" icon={<Icon name="reset" size={14} />} disabled={!canReset} onClick={stop(onReset)}>
                Reset
              </GlassButton>
            )}
          </>
        )}
      </footer>
    </article>
  );
}
