import { useEffect, useState } from "react";
import { GlassButton } from "../../components/glass/GlassButton";
import { Icon } from "../../components/Icon";
import { api, SERVER_TYPE_LABELS, type Instance, type JoinInfo, type Snapshot } from "../../lib/api";
import { formatBytes, formatPercent, formatUptime } from "../../lib/format";
import { useStore } from "../../state/store";

interface OverviewTabProps {
  instance: Instance;
  snap: Snapshot;
}

export function OverviewTab({ instance, snap }: OverviewTabProps) {
  const [join, setJoin] = useState<JoinInfo | null>(null);
  const playit = useStore((s) => s.playit);
  const tunnel = playit?.tunnels.find((t) => t.port === instance.port);
  const running = snap.state === "starting" || snap.state === "online" || snap.state === "stopping";

  useEffect(() => {
    api.joinInfo(instance.id).then(setJoin).catch(() => setJoin(null));
  }, [instance.id, instance.port, snap.state]);

  return (
    <div className="stack" style={{ gap: 16 }}>
      <div className="stat-grid">
        <Stat label="CPU" value={running ? formatPercent(snap.cpuPercent) : "—"} />
        <Stat label="Memory" value={running && snap.memoryBytes ? formatBytes(snap.memoryBytes) : "—"} />
        <Stat label="Players" value={`${snap.players.length} / ${instance.maxPlayers}`} />
        <Stat label="Uptime" value={running ? formatUptime(snap.uptimeSecs) : "—"} />
      </div>

      <section className="surface panel">
        <h2 className="section-title">Join addresses</h2>
        <AddressRow label="This PC" value={join?.localhost ?? `localhost:${instance.port}`} />
        <AddressRow label="Same Wi-Fi / LAN" value={join?.lan ?? null} empty="No network connection found" />
        <AddressRow
          label="Friends anywhere (playit.gg)"
          value={tunnel?.state === "connected" ? tunnel.address : null}
          empty={publicHint(playit?.state, tunnel?.state, tunnel?.message)}
        />
      </section>

      <section className="surface panel">
        <h2 className="section-title">Server</h2>
        <dl className="facts">
          <dt>Type</dt>
          <dd>
            {SERVER_TYPE_LABELS[instance.serverType]} {instance.mcVersion}
          </dd>
          <dt>Java</dt>
          <dd>{instance.javaMajor ? `Java ${instance.javaMajor}+` : "—"}</dd>
          <dt>Memory</dt>
          <dd>{(instance.ramMb / 1024).toFixed(instance.ramMb % 1024 ? 1 : 0)} GB</dd>
          <dt>World</dt>
          <dd className="mono">{instance.currentWorld ?? "Created on first launch"}</dd>
        </dl>
        {instance.serverType !== "vanilla" && (
          <div style={{ marginTop: 16 }}>
            <GlassButton size="sm" icon={<Icon name="folder" size={14} />} onClick={() => api.openAddonsFolder(instance.id).catch(() => {})}>
              {instance.serverType === "paper" ? "Open plugins folder" : "Open mods folder"}
            </GlassButton>
          </div>
        )}
      </section>
    </div>
  );
}

function publicHint(link?: string, tunnel?: string, message?: string | null): string {
  if (!link || link === "notSetUp" || link === "installing" || link === "waitingForClaim") {
    return "Set up playit.gg (sidebar) to get a public address";
  }
  if (link === "agentOffline") return "playit.gg agent offline — it restarts on its own";
  if (tunnel === "pending") return "Tunnel pending — the address appears in a moment";
  if (tunnel === "limitReached" || tunnel === "error") return message ?? "The tunnel could not be created";
  return "Launch the server to create its public address";
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="surface stat">
      <div className="stat-label">{label}</div>
      <div className="stat-value">{value}</div>
    </div>
  );
}

export function AddressRow({ label, value, empty }: { label: string; value: string | null; empty?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard access can be refused; the address is still visible to copy by hand.
    }
  };
  return (
    <div className="setting-row">
      <div>
        <div className="hint">{label}</div>
        <div className={value ? "mono address" : "muted"}>{value ?? empty}</div>
      </div>
      {value && (
        <GlassButton size="sm" icon={<Icon name={copied ? "check" : "copy"} size={14} />} onClick={copy} aria-label={`Copy ${label} address`}>
          {copied ? "Copied" : "Copy"}
        </GlassButton>
      )}
    </div>
  );
}
