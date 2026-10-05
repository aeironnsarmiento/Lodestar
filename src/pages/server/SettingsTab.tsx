import { useEffect, useState, type ReactNode } from "react";
import { GlassButton } from "../../components/glass/GlassButton";
import { GlassInput, GlassSelect, NumberInput, Switch } from "../../components/glass/GlassInput";
import { PlayerListEditor } from "../../components/PlayerListEditor";
import {
  api,
  type Difficulty,
  type GameMode,
  type Instance,
  type LevelType,
  type ServerProperties,
} from "../../lib/api";
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

function Panel({ title, note, children, className }: { title: string; note?: string; children: ReactNode; className?: string }) {
  return (
    <section className={["surface panel", className].filter(Boolean).join(" ")} aria-label={title}>
      <header className="panel-head">
        <h2 className="section-title">{title}</h2>
        {note && <span className="panel-note">{note}</span>}
      </header>
      {children}
    </section>
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

  const setProp = <K extends keyof ServerProperties>(key: K, value: ServerProperties[K]) => {
    setSaved(false);
    setDraft((d) => ({ ...d, properties: { ...d.properties, [key]: value } }));
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

  const p = draft.properties;
  const listsNote = running ? "Changes apply right away while the server is online" : undefined;

  return (
    <div className="stack settings-tab" style={{ gap: 16 }}>
      <Panel title="General">
        <Row label="Name">
          <GlassInput aria-label="Name" value={draft.name} onChange={(e) => set("name", e.target.value)} />
        </Row>
        <Row label="Message of the day" hint="Shown under the name in the server list">
          <GlassInput aria-label="Message of the day" value={draft.motd} onChange={(e) => set("motd", e.target.value)} />
        </Row>
        <Row label="Port" hint="Servers running at the same time need different ports">
          <NumberInput aria-label="Port" min={1024} max={65535} value={draft.port} onValue={(v) => set("port", v)} />
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
        <Row label="Start with the app" hint="Launch this server when Lodestar opens">
          <Switch label="Start with the app" checked={draft.autoStart} onChange={(v) => set("autoStart", v)} />
        </Row>
        {draft.serverType === "fabric" && (
          <Row label="Speed mods" hint="Lithium and FerriteCore, installed during setup">
            <Switch label="Speed mods" checked={draft.speedMods} onChange={(v) => set("speedMods", v)} />
          </Row>
        )}
      </Panel>

      <Panel title="Gameplay">
        <Row label="Game mode">
          <GlassSelect aria-label="Game mode" value={draft.gameMode} onChange={(e) => set("gameMode", e.target.value as GameMode)}>
            <option value="survival">Survival</option>
            <option value="creative">Creative</option>
            <option value="adventure">Adventure</option>
            <option value="spectator">Spectator</option>
          </GlassSelect>
        </Row>
        <Row label="Force game mode" hint="Put players back in the default game mode every time they join">
          <Switch label="Force game mode" checked={p.forceGamemode} onChange={(v) => setProp("forceGamemode", v)} />
        </Row>
        <Row label="Difficulty" hint={draft.hardcore ? "Hardcore always plays on Hard" : undefined}>
          <GlassSelect
            aria-label="Difficulty"
            value={draft.hardcore ? "hard" : draft.difficulty}
            disabled={draft.hardcore}
            onChange={(e) => set("difficulty", e.target.value as Difficulty)}
          >
            <option value="peaceful">Peaceful</option>
            <option value="easy">Easy</option>
            <option value="normal">Normal</option>
            <option value="hard">Hard</option>
          </GlassSelect>
        </Row>
        <Row label="Hardcore" hint="One life; best chosen when creating or resetting a world">
          <Switch label="Hardcore" checked={draft.hardcore} onChange={(v) => set("hardcore", v)} />
        </Row>
        <Row label="PvP" hint="Players can hurt each other">
          <Switch label="PvP" checked={p.pvp} onChange={(v) => setProp("pvp", v)} />
        </Row>
        <Row label="Allow flight" hint="Keeps lag or flying mods from getting players kicked">
          <Switch label="Allow flight" checked={p.allowFlight} onChange={(v) => setProp("allowFlight", v)} />
        </Row>
        <Row label="Command blocks">
          <Switch label="Command blocks" checked={p.enableCommandBlock} onChange={(v) => setProp("enableCommandBlock", v)} />
        </Row>
      </Panel>

      <Panel title="World generation" note="World type and structures apply to new worlds">
        <Row label="World type">
          <GlassSelect aria-label="World type" value={p.levelType} onChange={(e) => setProp("levelType", e.target.value as LevelType)}>
            <option value="normal">Normal</option>
            <option value="flat">Superflat</option>
            <option value="largeBiomes">Large biomes</option>
            <option value="amplified">Amplified</option>
          </GlassSelect>
        </Row>
        <Row label="Generate structures" hint="Villages, strongholds, temples and the like">
          <Switch label="Generate structures" checked={p.generateStructures} onChange={(v) => setProp("generateStructures", v)} />
        </Row>
        <Row label="Allow the Nether">
          <Switch label="Allow the Nether" checked={p.allowNether} onChange={(v) => setProp("allowNether", v)} />
        </Row>
        <Row label="Spawn protection" hint="Blocks around spawn only operators can change (0 turns it off)">
          <NumberInput aria-label="Spawn protection" min={0} max={1000} value={p.spawnProtection} onValue={(v) => setProp("spawnProtection", v)} />
        </Row>
      </Panel>

      <Panel title="Players">
        <Row label="Player limit">
          <NumberInput aria-label="Player limit" min={1} max={1000} value={draft.maxPlayers} onValue={(v) => set("maxPlayers", v)} />
        </Row>
        <Row label="Online mode" hint="Check accounts with Mojang (turn off only for offline LAN play)">
          <Switch label="Online mode" checked={draft.onlineMode} onChange={(v) => set("onlineMode", v)} />
        </Row>
        <Row label="Require signed chat" hint="Reject players whose chat is not signed by Mojang">
          <Switch label="Require signed chat" checked={p.enforceSecureProfile} onChange={(v) => setProp("enforceSecureProfile", v)} />
        </Row>
        <Row label="Hide player list" hint="Do not show who is online in the server list">
          <Switch label="Hide player list" checked={p.hideOnlinePlayers} onChange={(v) => setProp("hideOnlinePlayers", v)} />
        </Row>
        <Row label="Idle kick" hint="Minutes before idle players are kicked (0 never kicks)">
          <NumberInput aria-label="Idle kick" min={0} max={1440} value={p.playerIdleTimeout} onValue={(v) => setProp("playerIdleTimeout", v)} />
        </Row>
      </Panel>

      <Panel title="Whitelist" note={listsNote}>
        <Row label="Use the whitelist" hint="Only players on the list can join">
          <Switch label="Use the whitelist" checked={p.whiteList} onChange={(v) => setProp("whiteList", v)} />
        </Row>
        <Row label="Kick players not on it" hint="Removes anyone online who is not on the list when it changes">
          <Switch
            label="Kick players not on it"
            checked={p.enforceWhitelist}
            disabled={!p.whiteList}
            onChange={(v) => setProp("enforceWhitelist", v)}
          />
        </Row>
        <PlayerListEditor
          label="Whitelisted players"
          names={draft.whitelist}
          empty="Nobody is on the whitelist yet."
          onChange={(names) => set("whitelist", names)}
        />
      </Panel>

      <Panel title="Operators" note={listsNote}>
        <Row label="Your Minecraft name" hint="Made an operator whenever the server comes online">
          <GlassInput
            aria-label="Your Minecraft name"
            placeholder="e.g. Steve"
            value={draft.opName}
            onChange={(e) => set("opName", e.target.value.trim())}
          />
        </Row>
        <PlayerListEditor
          label="Other operators"
          names={draft.operators}
          empty="No other operators."
          onChange={(names) => set("operators", names)}
        />
      </Panel>

      <Panel title="Performance">
        <Row label="View distance" hint="Chunks sent to players">
          <NumberInput aria-label="View distance" min={2} max={32} value={draft.viewDistance} onValue={(v) => set("viewDistance", v)} />
        </Row>
        <Row label="Simulation distance" hint="Chunks that tick around players">
          <NumberInput
            aria-label="Simulation distance"
            min={2}
            max={32}
            value={draft.simulationDistance}
            onValue={(v) => set("simulationDistance", v)}
          />
        </Row>
        <Row label="Entity range" hint="How far away entities are sent to players, in percent">
          <NumberInput
            aria-label="Entity range"
            min={10}
            max={1000}
            value={p.entityBroadcastRangePercentage}
            onValue={(v) => setProp("entityBroadcastRangePercentage", v)}
          />
        </Row>
        <Row label="Safe chunk writes" hint="Slower, but no lost chunks if the PC loses power">
          <Switch label="Safe chunk writes" checked={p.syncChunkWrites} onChange={(v) => setProp("syncChunkWrites", v)} />
        </Row>
      </Panel>

      <Panel title="Resource pack">
        <Row label="Download link" hint="A direct link to a .zip resource pack">
          <GlassInput
            aria-label="Resource pack link"
            placeholder="https://…"
            value={p.resourcePack}
            onChange={(e) => setProp("resourcePack", e.target.value)}
          />
        </Row>
        <Row label="Require it" hint="Players who decline the pack are disconnected">
          <Switch
            label="Require the resource pack"
            checked={p.requireResourcePack}
            disabled={!p.resourcePack.trim()}
            onChange={(v) => setProp("requireResourcePack", v)}
          />
        </Row>
      </Panel>

      <Panel title="Scheduled restarts">
        <ScheduleEditor value={draft.restart} onChange={(restart) => set("restart", restart)} />
      </Panel>

      <div className="save-bar surface" data-floating={dirty}>
        <span className="muted">
          {error ? (
            <span className="error-text" role="alert">
              {error}
            </span>
          ) : saved ? (
            "Saved. Changes apply on the next start."
          ) : dirty ? (
            "You have unsaved changes."
          ) : running ? (
            "Changes apply on the next start."
          ) : (
            ""
          )}
        </span>
        <GlassButton variant="primary" disabled={!dirty} onClick={save}>
          Save changes
        </GlassButton>
      </div>

      <Panel title="Delete server" className="danger-zone">
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
      </Panel>
    </div>
  );
}
