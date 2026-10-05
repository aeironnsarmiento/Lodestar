import { useEffect, useState } from "react";
import { GlassButton } from "../components/glass/GlassButton";
import { api, type Instance, type ProjectVersion } from "../lib/api";
import { errorMessage, refreshInstances } from "../state/store";
import { Dialog } from "./Dialog";

interface ModpackVersionsDialogProps {
  instance: Instance;
  onClose: () => void;
}

/** Switches a modpack server to another version of its pack (update or roll back). */
export function ModpackVersionsDialog({ instance, onClose }: ModpackVersionsDialogProps) {
  const pack = instance.modpack!;
  const [versions, setVersions] = useState<ProjectVersion[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!pack.source || !pack.projectId) return;
    api
      .projectVersions(pack.source, pack.projectId, [], null)
      .then(setVersions)
      .catch((e) => setError(errorMessage(e)));
  }, [pack.source, pack.projectId]);

  const choose = async (v: ProjectVersion) => {
    setBusy(true);
    setError(null);
    try {
      await api.updateModpack(instance.id, v.versionId, v.versionNumber);
      await refreshInstances();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  return (
    <Dialog title={`${pack.title} versions`} onClose={onClose} width={560}>
      <p>
        Switching reinstalls the pack's files and settings. Mods you added yourself stay; your worlds are not touched.
      </p>
      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      {!versions && !error && <p className="faint">Loading versions…</p>}
      <div className="browser-versions" style={{ maxHeight: 360 }}>
        {versions?.map((v) => {
          const current = v.versionId === pack.versionId;
          return (
            <div key={v.versionId} className="browser-version">
              <div style={{ minWidth: 0 }}>
                <div className="browser-version-name">
                  {v.versionNumber}
                  {current && <span className="tag">Installed</span>}
                  {v.channel !== "release" && <span className="tag quiet">{v.channel}</span>}
                </div>
                <div className="faint mono browser-version-meta">
                  {v.loaders.join(", ")} · {v.gameVersions.join(", ")}
                </div>
              </div>
              <GlassButton size="sm" variant={current ? "default" : "primary"} disabled={busy || current} onClick={() => choose(v)}>
                {current ? "Current" : "Switch"}
              </GlassButton>
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}
