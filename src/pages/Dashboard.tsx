import { useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { ServerCard } from "../components/ServerCard";
import { GlassButton } from "../components/glass/GlassButton";
import { Icon } from "../components/Icon";
import { NewServerDialog } from "../dialogs/NewServerDialog";
import { DeleteServerDialog } from "../dialogs/DeleteServerDialog";
import { api } from "../lib/api";
import { refreshInstances, stoppedSnapshot, useStore } from "../state/store";
import { requestLaunch, resetWorld, runAction } from "../state/actions";

interface DashboardProps {
  onOpen: (id: string) => void;
  onOpenSettings?: () => void;
}

export function Dashboard({ onOpen, onOpenSettings }: DashboardProps) {
  const instances = useStore((s) => s.instances);
  const runtime = useStore((s) => s.runtime);
  const progress = useStore((s) => s.progress);
  const actionErrors = useStore((s) => s.actionErrors);
  const tunnels = useStore((s) => s.playit?.tunnels);
  const [creating, setCreating] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const deleting = instances.find((i) => i.id === deletingId);

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
            tunnel={tunnels?.find((t) => t.port === inst.port)}
            onOpen={() => onOpen(inst.id)}
            onLaunch={() => requestLaunch(inst.id)}
            onStop={() => runAction(inst.id, () => api.stopServer(inst.id))}
            onRestart={() => runAction(inst.id, () => api.restartServer(inst.id))}
            onKill={() => runAction(inst.id, () => api.killServer(inst.id))}
            onReset={(seed, hardcore) => resetWorld(inst.id, seed, hardcore)}
            onRetry={() => runAction(inst.id, () => api.retryProvision(inst.id))}
            onDelete={() => setDeletingId(inst.id)}
          />
        ))}
        <button type="button" className="new-server-card" onClick={() => setCreating(true)}>
          <Icon name="plus" size={26} />
          New server
        </button>
      </div>
      {deleting && <DeleteServerDialog instance={deleting} onClose={() => setDeletingId(null)} />}
      {creating && (
        <NewServerDialog
          onOpenSettings={onOpenSettings}
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
