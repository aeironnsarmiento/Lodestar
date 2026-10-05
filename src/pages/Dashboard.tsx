import { useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { ServerCard } from "../components/ServerCard";
import { GlassButton } from "../components/glass/GlassButton";
import { Icon } from "../components/Icon";
import { NewServerDialog } from "../dialogs/NewServerDialog";
import { api } from "../lib/api";
import { refreshInstances, stoppedSnapshot, useStore } from "../state/store";
import { requestLaunch, requestReset, runAction } from "../state/actions";

interface DashboardProps {
  onOpen: (id: string) => void;
}

export function Dashboard({ onOpen }: DashboardProps) {
  const instances = useStore((s) => s.instances);
  const runtime = useStore((s) => s.runtime);
  const progress = useStore((s) => s.progress);
  const actionErrors = useStore((s) => s.actionErrors);
  const [creating, setCreating] = useState(false);

  const online = instances.filter((i) => runtime[i.id]?.state === "online").length;

  return (
    <div className="page">
      <PageHeader
        title="Dashboard"
        subtitle={instances.length ? `${instances.length} servers · ${online} online` : "Your Minecraft servers"}
        actions={
          <GlassButton variant="primary" icon={<Icon name="plus" size={16} />} onClick={() => setCreating(true)}>
            New server
          </GlassButton>
        }
      />
      <div className="card-grid">
        {instances.map((inst) => (
          <ServerCard
            key={inst.id}
            instance={inst}
            snap={runtime[inst.id] ?? stoppedSnapshot(inst.id, inst.port)}
            progress={progress[inst.id]}
            error={actionErrors[inst.id]}
            onOpen={() => onOpen(inst.id)}
            onLaunch={() => requestLaunch(inst.id)}
            onStop={() => runAction(inst.id, () => api.stopServer(inst.id))}
            onReset={(seed) => requestReset(inst.id, seed)}
            onRetry={() => runAction(inst.id, () => api.retryProvision(inst.id))}
          />
        ))}
        <button type="button" className="new-server-card" onClick={() => setCreating(true)}>
          <Icon name="plus" size={26} />
          New server
        </button>
      </div>
      {creating && (
        <NewServerDialog
          onClose={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            refreshInstances();
          }}
        />
      )}
    </div>
  );
}
