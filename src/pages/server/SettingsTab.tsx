import { useEffect, useState, type ReactNode } from "react";
import { GlassButton } from "../../components/glass/GlassButton";
import { GlassInput, GlassSelect, NumberInput, Switch } from "../../components/glass/GlassInput";
import { api, type Difficulty, type GameMode, type Instance } from "../../lib/api";
import { ScheduleEditor } from "../../components/ScheduleEditor";
import { errorMessage } from "../../state/store";

interface SettingsTabProps {
  instance: Instance;
  running: boolean;
  onDeleted: () => void;
}

const RAM_CHOICES = [1, 2, 3, 4, 6, 8, 10, 12, 16];

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div>
        <div className="label">{label}</div>
        {hint && <div className="hint">{hint}</div>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

export function SettingsTab({ instance, running, onDeleted }: SettingsTabProps) {
  const [draft, setDraft] = useState<Instance>(instance);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Pick up backend changes (provisioning, world switches) when nothing is being edited.
  const dirty = JSON.stringify(draft) !== JSON.stringify(instance);
  useEffect(() => {
    if (!dirty) setDraft(instance);
  }, [instance]); // `dirty` is read on purpose without re-running on every keystroke

  const set = <K extends keyof Instance>(key: K, value: Instance[K]) => {
    setSaved(false);
    setDraft((d) => ({ ...d, [key]: value }));
  };

  const save = async () => {
    setError(null);
    try {
      const next = await api.updateInstance(draft);
      setDraft(next);
      setSaved(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const remove = async () => {
    setError(null);
    try {
      await api.deleteInstance(instance.id);
      onDeleted();
    } catch (e) {
      setError(errorMessage(e));
      setConfirmDelete(false);
    }
  };

  return (
    <div className="stack" style={{ gap: 16 }}>
      <section className="surface panel">
        <h2 className="section-title">World</h2>
        <Row label="Game mode">
          <GlassSelect aria-label="Game mode" value={draft.gameMode} onChange={(e) => set("gameMode", e.target.value as GameMode)}>
            <option value="survival">Survival</option>
            <option value="creative">Creative</option>
            <option value="adventure">Adventure</option>
            <option value="spectator">Spectator</option>
          </GlassSelect>
        </Row>
        <Row label="Difficulty">
          <GlassSelect aria-label="Difficulty" value={draft.difficulty} onChange={(e) => set("difficulty", e.target.value as Difficulty)}>
            <option value="peaceful">Peaceful</option>
            <option value="easy">Easy</option>
            <option value="normal">Normal</option>
            <option value="hard">Hard</option>
          </GlassSelect>
        </Row>
        <Row label="Hardcore">
          <Switch label="Hardcore" checked={draft.hardcore} onChange={(v) => set("hardcore", v)} />
        </Row>
        <Row label="Player limit">
          <NumberInput aria-label="Player limit" min={1} max={1000} value={draft.maxPlayers} onValue={(v) => set("maxPlayers", v)} />
        </Row>
        <Row label="View distance" hint="Chunks sent to players">
          <NumberInput aria-label="View distance" min={2} max={32} value={draft.viewDistance} onValue={(v) => set("viewDistance", v)} />
        </Row>
        <Row label="Simulation distance" hint="Chunks that tick around players">
          <NumberInput aria-label="Simulation distance" min={2} max={32} value={draft.simulationDistance} onValue={(v) => set("simulationDistance", v)} />
        </Row>
      </section>

      <section className="surface panel">
        <h2 className="section-title">Server</h2>
        <Row label="Name">
          <GlassInput aria-label="Name" value={draft.name} onChange={(e) => set("name", e.target.value)} />
        </Row>
        <Row label="Message of the day">
          <GlassInput aria-label="Message of the day" value={draft.motd} onChange={(e) => set("motd", e.target.value)} />
        </Row>
        <Row label="Memory" hint="RAM the server may use">
          <GlassSelect aria-label="Memory" value={draft.ramMb} onChange={(e) => set("ramMb", Number(e.target.value))}>
            {RAM_CHOICES.map((gb) => (
              <option key={gb} value={gb * 1024}>
                {gb} GB
              </option>
            ))}
          </GlassSelect>
        </Row>
        <Row label="Port" hint="Servers running at the same time need different ports">
          <NumberInput aria-label="Port" min={1024} max={65535} value={draft.port} onValue={(v) => set("port", v)} />
        </Row>
        <Row label="Online mode" hint="Check accounts with Mojang (turn off only for offline LAN play)">
          <Switch label="Online mode" checked={draft.onlineMode} onChange={(v) => set("onlineMode", v)} />
        </Row>
        <Row label="Your Minecraft name" hint="Made an operator whenever the server comes online">
          <GlassInput aria-label="Your Minecraft name" placeholder="e.g. Steve" value={draft.opName} onChange={(e) => set("opName", e.target.value.trim())} />
        </Row>
        <Row label="Start with the app" hint="Launch this server when Lodestar opens">
          <Switch label="Start with the app" checked={draft.autoStart} onChange={(v) => set("autoStart", v)} />
        </Row>
        {draft.serverType === "fabric" && (
          <Row label="Speed mods" hint="Lithium and FerriteCore, installed during setup">
            <Switch label="Speed mods" checked={draft.speedMods} onChange={(v) => set("speedMods", v)} />
          </Row>
        )}
      </section>

      <section className="surface panel">
        <h2 className="section-title">Scheduled restarts</h2>
        <ScheduleEditor value={draft.restart} onChange={(restart) => set("restart", restart)} />
      </section>

      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      <div className="row" style={{ justifyContent: "space-between" }}>
        <span className="muted">{saved ? "Saved. Changes apply on the next start." : running ? "Changes apply on the next start." : ""}</span>
        <GlassButton variant="primary" disabled={!dirty} onClick={save}>
          Save changes
        </GlassButton>
      </div>

      <section className="surface panel danger-zone">
        <h2 className="section-title">Delete server</h2>
        <Row label="Delete this server" hint="Removes its files and every kept world. This cannot be undone.">
          {confirmDelete ? (
            <span className="row">
              <GlassButton size="sm" onClick={() => setConfirmDelete(false)}>
                Cancel
              </GlassButton>
              <GlassButton size="sm" variant="danger" onClick={remove}>
                Delete forever
              </GlassButton>
            </span>
          ) : (
            <GlassButton size="sm" variant="danger" disabled={running} onClick={() => setConfirmDelete(true)}>
              Delete
            </GlassButton>
          )}
        </Row>
      </section>
    </div>
  );
}
