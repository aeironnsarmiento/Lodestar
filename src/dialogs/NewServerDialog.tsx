import { useEffect, useMemo, useState, type FormEvent } from "react";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassInput, GlassSelect, NumberInput, Switch } from "../components/glass/GlassInput";
import {
  api,
  SERVER_TYPE_LABELS,
  type Difficulty,
  type GameMode,
  type Instance,
  type ServerType,
  type VersionEntry,
} from "../lib/api";
import { formatDate } from "../lib/format";
import { visibleVersions } from "../lib/versions";
import { errorMessage } from "../state/store";
import { Dialog } from "./Dialog";
import { ModpackServerForm } from "./ModpackServerForm";

const TYPES: ServerType[] = ["vanilla", "paper", "fabric", "forge", "neoforge"];

const TYPE_HINTS: Record<ServerType, string> = {
  vanilla: "Mojang's own server",
  paper: "Fast, plugin support",
  fabric: "Speedrun favourite; speed mods installed automatically",
  forge: "Forge mods, Minecraft 1.17+",
  neoforge: "NeoForge mods, Minecraft 1.20.2+",
};

interface NewServerDialogProps {
  onClose: () => void;
  onCreated: (instance: Instance) => void;
  onOpenSettings?: () => void;
}

export function NewServerDialog({ onClose, onCreated, onOpenSettings }: NewServerDialogProps) {
  const [mode, setMode] = useState<"blank" | "modpack">("blank");
  const [name, setName] = useState("Speedrun server");
  const [type, setType] = useState<ServerType>("fabric");
  const [versions, setVersions] = useState<VersionEntry[] | null>(null);
  const [versionsError, setVersionsError] = useState<string | null>(null);
  const [showSnapshots, setShowSnapshots] = useState(false);
  const [version, setVersion] = useState("");
  const [seed, setSeed] = useState("");
  const [gameMode, setGameMode] = useState<GameMode>("survival");
  const [difficulty, setDifficulty] = useState<Difficulty>("easy");
  const [maxPlayers, setMaxPlayers] = useState(10);
  const [hardcore, setHardcore] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setVersions(null);
    setVersionsError(null);
    api
      .listVersions(type)
      .then((list) => !cancelled && setVersions(list))
      .catch((e) => !cancelled && setVersionsError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [type]);

  const shown = useMemo(() => visibleVersions(type, versions ?? [], showSnapshots), [type, versions, showSnapshots]);

  // Keep a valid selection when the list changes.
  useEffect(() => {
    if (shown.length && !shown.some((v) => v.id === version)) setVersion(shown[0].id);
    if (!shown.length) setVersion("");
  }, [shown, version]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !version) return;
    setBusy(true);
    setError(null);
    try {
      const inst = await api.createInstance({
        name: name.trim(),
        serverType: type,
        mcVersion: version,
        seed: seed.trim() || null,
        gameMode,
        difficulty: hardcore ? "hard" : difficulty,
        hardcore,
        maxPlayers,
      });
      onCreated(inst);
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <Dialog title="New server" onClose={onClose} width={600}>
      <div className="segmented new-server-mode" role="radiogroup" aria-label="Start from">
        {(
          [
            ["blank", "A fresh server"],
            ["modpack", "A modpack"],
          ] as const
        ).map(([m, label]) => (
          <button
            key={m}
            type="button"
            role="radio"
            aria-checked={mode === m}
            className={mode === m ? "segment active" : "segment"}
            onClick={() => setMode(m)}
          >
            {label}
          </button>
        ))}
      </div>
      {mode === "modpack" ? (
        <ModpackServerForm onClose={onClose} onCreated={onCreated} onOpenSettings={onOpenSettings} />
      ) : (
      <form className="stack" onSubmit={submit} aria-label="New server">
        <div className="field">
          <label htmlFor="ns-name">Name</label>
          <GlassInput id="ns-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </div>

        <div className="field">
          <span className="label" id="ns-type">
            Server type
          </span>
          <div className="segmented" role="radiogroup" aria-labelledby="ns-type">
            {TYPES.map((t) => (
              <button
                key={t}
                type="button"
                role="radio"
                aria-checked={type === t}
                className={type === t ? "segment active" : "segment"}
                onClick={() => setType(t)}
              >
                {SERVER_TYPE_LABELS[t]}
              </button>
            ))}
          </div>
          <span className="faint" style={{ fontSize: 12.5 }}>
            {TYPE_HINTS[type]}
          </span>
        </div>

        <div className="field">
          <div className="row" style={{ justifyContent: "space-between" }}>
            <label htmlFor="ns-version">Minecraft version</label>
            <label className="row faint" style={{ fontSize: 12.5, gap: 6 }}>
              <input type="checkbox" checked={showSnapshots} onChange={(e) => setShowSnapshots(e.target.checked)} />
              Show snapshots
            </label>
          </div>
          <GlassSelect
            id="ns-version"
            value={version}
            onChange={(e) => setVersion(e.target.value)}
            disabled={!versions || shown.length === 0}
          >
            {!versions && !versionsError && <option value="">Loading versions…</option>}
            {versions && shown.length === 0 && <option value="">No versions available</option>}
            {shown.map((v) => (
              <option key={v.id} value={v.id}>
                {v.id}
                {v.kind === "snapshot" ? " (snapshot)" : ""}
                {v.releaseTime ? ` — ${formatDate(v.releaseTime)}` : ""}
              </option>
            ))}
          </GlassSelect>
          {versionsError && <span className="error-text">Could not load versions: {versionsError}</span>}
        </div>

        <div className="field-grid">
          <div className="field">
            <label htmlFor="ns-seed">Seed</label>
            <GlassInput id="ns-seed" placeholder="Random" value={seed} onChange={(e) => setSeed(e.target.value)} />
          </div>
          <div className="field">
            <label htmlFor="ns-mode">Game mode</label>
            <GlassSelect id="ns-mode" value={gameMode} onChange={(e) => setGameMode(e.target.value as GameMode)}>
              <option value="survival">Survival</option>
              <option value="creative">Creative</option>
              <option value="adventure">Adventure</option>
              <option value="spectator">Spectator</option>
            </GlassSelect>
          </div>
          <div className="field">
            <label htmlFor="ns-difficulty">Difficulty</label>
            <GlassSelect
              id="ns-difficulty"
              value={hardcore ? "hard" : difficulty}
              disabled={hardcore}
              onChange={(e) => setDifficulty(e.target.value as Difficulty)}
            >
              <option value="peaceful">Peaceful</option>
              <option value="easy">Easy</option>
              <option value="normal">Normal</option>
              <option value="hard">Hard</option>
            </GlassSelect>
          </div>
          <div className="field">
            <label htmlFor="ns-players">Player limit</label>
            <NumberInput id="ns-players" min={1} max={100} value={maxPlayers} onValue={setMaxPlayers} />
          </div>
        </div>

        <div className="setting-row toggle-row">
          <div>
            <div className="label">Hardcore</div>
            <div className="hint">One life: dying puts players in spectator mode. Difficulty is locked to Hard.</div>
          </div>
          <Switch label="Hardcore" checked={hardcore} onChange={setHardcore} />
        </div>

        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        <div className="dialog-footer">
          <GlassButton onClick={onClose}>Cancel</GlassButton>
          <GlassButton type="submit" variant="primary" disabled={busy || !name.trim() || !version}>
            Create server
          </GlassButton>
        </div>
      </form>
      )}
    </Dialog>
  );
}
