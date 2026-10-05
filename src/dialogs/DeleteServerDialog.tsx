import { useState } from "react";
import { GlassButton } from "../components/glass/GlassButton";
import { api, type Instance } from "../lib/api";
import { errorMessage, refreshInstances } from "../state/store";
import { Dialog } from "./Dialog";

interface DeleteServerDialogProps {
  instance: Instance;
  onClose: () => void;
  onDeleted?: () => void;
}

/** Confirms deleting a server with its files and every kept world. */
export function DeleteServerDialog({ instance, onClose, onDeleted }: DeleteServerDialogProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.deleteInstance(instance.id);
      await refreshInstances();
      onDeleted?.();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={`Delete ${instance.name}?`}
      onClose={onClose}
      width={440}
      footer={
        <>
          <GlassButton onClick={onClose}>Cancel</GlassButton>
          <GlassButton variant="danger" disabled={busy} onClick={remove}>
            Delete forever
          </GlassButton>
        </>
      }
    >
      <p>This removes the server, its settings, mods or plugins, and every kept world. It cannot be undone.</p>
      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
