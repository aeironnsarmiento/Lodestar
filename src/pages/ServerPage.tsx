import { useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { StatusBadge } from "../components/StatusBadge";
import { GlassButton } from "../components/glass/GlassButton";
import { Icon } from "../components/Icon";
import { api, SERVER_TYPE_LABELS, type Instance, type Snapshot } from "../lib/api";
import { stoppedSnapshot, useStore } from "../state/store";
import { requestLaunch, requestReset, runAction } from "../state/actions";
import { ResetButton } from "../dialogs/ResetSeedPopover";
import { badgeState } from "../components/ServerCard";
import { OverviewTab } from "./server/OverviewTab";
import { ConsoleTab } from "./server/ConsoleTab";
import { SettingsTab } from "./server/SettingsTab";
import { WorldsTab } from "./server/WorldsTab";

export type ServerTab = "overview" | "console" | "worlds" | "settings";

const TABS: { id: ServerTab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "console", label: "Console" },
  { id: "worlds", label: "Worlds" },
  { id: "settings", label: "Settings" },
];

interface ServerPageProps {
  id: string;
  onBack: () => void;
  initialTab?: ServerTab;
}

export function ServerPage({ id, onBack, initialTab = "overview" }: ServerPageProps) {
  const instance = useStore((s) => s.instances.find((i) => i.id === id));
  const runtime = useStore((s) => s.runtime[id]);
  const [tab, setTab] = useState<ServerTab>(initialTab);
  const error = useStore((s) => s.actionErrors[id]);

  if (!instance) {
    return (
      <div className="page">
        <PageHeader title="Server not found" leading={<BackButton onBack={onBack} />} />
      </div>
    );
  }
  const snap = runtime ?? stoppedSnapshot(id, instance.port);
  const running = snap.state !== "stopped" && snap.state !== "crashed";

  return (
    <div className="page">
      <PageHeader
        leading={<BackButton onBack={onBack} />}
        title={
          <span className="row" style={{ gap: 10 }}>
            {instance.name} <StatusBadge state={badgeState(instance, snap)} />
          </span>
        }
        subtitle={`${SERVER_TYPE_LABELS[instance.serverType]} ${instance.mcVersion} · port ${instance.port}`}
        actions={<ServerActions instance={instance} snap={snap} />}
      />
      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      {!error && snap.message && snap.state !== "online" && <p className="muted">{snap.message}</p>}
      <div className="tabs" role="tablist" aria-label="Server sections">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "tab active" : "tab"}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab === "overview" && <OverviewTab instance={instance} snap={snap} />}
        {tab === "console" && <ConsoleTab instance={instance} snap={snap} />}
        {tab === "worlds" && <WorldsTab instance={instance} />}
        {tab === "settings" && <SettingsTab instance={instance} running={running} onDeleted={onBack} />}
      </div>
    </div>
  );
}

function BackButton({ onBack }: { onBack: () => void }) {
  return <GlassButton iconOnly aria-label="Back to dashboard" icon={<Icon name="back" />} onClick={onBack} />;
}

interface ActionsProps {
  instance: Instance;
  snap: Snapshot;
}

function ServerActions({ instance, snap }: ActionsProps) {
  const s = snap.state;
  const ready = instance.provision.state === "ready";
  const run = (action: () => Promise<void>) => runAction(instance.id, action);
  const canReset = ready && (s === "stopped" || s === "online" || s === "crashed");
  return (
    <div className="row">
      <ResetButton disabled={!canReset} onReset={(seed) => requestReset(instance.id, seed)} />
      {(s === "stopped" || s === "crashed") && (
        <GlassButton
          variant="primary"
          icon={<Icon name="play" size={14} />}
          disabled={!ready}
          onClick={() => requestLaunch(instance.id)}
        >
          Launch
        </GlassButton>
      )}
      {s === "online" && (
        <GlassButton icon={<Icon name="reset" size={16} />} onClick={() => run(() => api.restartServer(instance.id))}>
          Restart
        </GlassButton>
      )}
      {(s === "starting" || s === "online") && (
        <GlassButton icon={<Icon name="stop" size={14} />} onClick={() => run(() => api.stopServer(instance.id))}>
          Stop
        </GlassButton>
      )}
      {s === "stopping" && (
        <GlassButton variant="danger" icon={<Icon name="kill" size={16} />} onClick={() => run(() => api.killServer(instance.id))}>
          Force kill
        </GlassButton>
      )}
    </div>
  );
}
