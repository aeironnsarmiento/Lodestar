import { useEffect, useState, type ReactNode } from "react";
import { GlassButton } from "../../components/glass/GlassButton";
import { GlassInput, GlassSelect, NumberInput, Switch } from "../../components/glass/GlassInput";
import { GlassPanel } from "../../components/glass/GlassPanel";
import { Icon } from "../../components/Icon";
import { LoaderVersionSelect, useLoaderChoices } from "../../components/LoaderVersionSelect";
import { PlayerListEditor } from "../../components/PlayerListEditor";
import {
  api,
  hasLoader,
  SERVER_TYPE_LABELS,
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

/** Which Forge, NeoForge or Fabric build the server runs, and reinstalling another. */
function ServerSoftwarePanel({ instance, running }: { instance: Instance; running: boolean }) {
  const loader = SERVER_TYPE_LABELS[instance.serverType];
  const settingUp = instance.provision.state === "running";
  // Re-read the installed build and the mods once setup finishes.
  const { choices, error } = useLoaderChoices(instance.serverType, instance.mcVersion, instance.id, instance.provision.state);
  const [pick, setPick] = useState<string | null>(instance.loaderVersion);
  const [actionError, setActionError] = useState<string | null>(null);
  useEffect(() => setPick(instance.loaderVersion), [instance.loaderVersion]);

  const installed = choices?.installed ?? null;
  const target = pick ?? choices?.automatic ?? null;
  const pending = pick !== instance.loaderVersion || (target !== null && target !== installed);
  const installedRefusals =
    choices && installed ? (choices.versions.find((v) => v.id === installed)?.rejectedBy ?? []).map((i) => choices.requirements[i]) : [];

  const install = async () => {
    setActionError(null);
    try {
      await api.setLoaderVersion(instance.id, pick);
    } catch (e) {
      setActionError(errorMessage(e));
    }
  };

  const hint = [`Minecraft ${instance.mcVersion}`, installed ? `${installed} installed` : "not installed yet"].join(" · ");
  return (
    <Panel title="Server software" note={settingUp ? "Installing…" : running ? "Stop the server to change it" : undefined}>
      <Row label={`${loader} version`} hint={hint}>
        <div className="stack" style={{ gap: 6, alignItems: "flex-end" }}>
          <LoaderVersionSelect
            serverType={instance.serverType}
            choices={choices}
            error={error}
            value={pick}
            onChange={setPick}
            disabled={settingUp}
          />
          <GlassButton size="sm" variant="primary" disabled={!choices || !target || !pending || running || settingUp} onClick={install}>
            {target ? `Install ${target}` : "Install"}
          </GlassButton>
        </div>
      </Row>
      {installedRefusals.length > 0 && (
        <div className="stack" style={{ gap: 2, padding: "0 0 8px" }} role="alert">
          {installedRefusals.map((r) => (
            <span key={r.fileName + r.range} className="warn-text" style={{ fontSize: 12.5 }}>
              {r.modName} needs {loader} {r.summary}, so the server will not start on {installed}.
            </span>
          ))}
        </div>
      )}
      {instance.modpack && (
        <p className="hint" style={{ fontSize: 12.5 }}>
          {instance.modpack.title} chose this version; another one may not suit the pack.
        </p>
      )}
      {actionError && <p className="error-text">{actionError}</p>}
    </Panel>
  );
}

export function SettingsTab({ instance, running, onDeleted }: SettingsTabProps) {
  // `base` is the last saved record: the instance, or what a save just returned
  // before the backend's change event catches up.
  const [base, setBase] = useState<Instance>(instance);
  const [draft, setDraft] = useState<Instance>(instance);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Pick up backend changes (provisioning, world switches) when nothing is being edited.
  const dirty = JSON.stringify(draft) !== JSON.stringify(base);
  useEffect(() => {
    setBase(instance);
    if (!dirty) setDraft(instance);
  }, [instance]); // `dirty` is read on purpose without re-running on every keystroke

  // The "Saved" confirmation fades out on its own.
  useEffect(() => {
    if (!saved) return;
    const timer = setTimeout(() => setSaved(false), 2500);
    return () => clearTimeout(timer);
  }, [saved]);

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
      setBase(next);
      setDraft(next);
      setSaved(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const discard = () => {
    setError(null);
    setDraft(base);
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

      {hasLoader(instance.serverType) && <ServerSoftwarePanel instance={instance} running={running} />}

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

      <Panel title="Whitelist" note={p.whiteList ? listsNote : undefined}>
        <Row label="Use the whitelist" hint="Only players on the list can join">
          <Switch label="Use the whitelist" checked={p.whiteList} onChange={(v) => setProp("whiteList", v)} />
        </Row>
        {p.whiteList && (
          <div className="reveal">
            <Row label="Kick players not on it" hint="Removes anyone online who is not on the list when it changes">
              <Switch label="Kick players not on it" checked={p.enforceWhitelist} onChange={(v) => setProp("enforceWhitelist", v)} />
            </Row>
            <PlayerListEditor
              label="Whitelisted players"
              names={draft.whitelist}
              empty="Nobody is on the whitelist yet."
              onChange={(names) => set("whitelist", names)}
            />
          </div>
        )}
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
        <Row label="Restart on a schedule" hint="Daily restarts keep long-running servers fresh">
          <Switch
            label="Restart on a schedule"
            checked={draft.restart.enabled}
            onChange={(enabled) => set("restart", { ...draft.restart, enabled })}
          />
        </Row>
        {draft.restart.enabled && (
          <div className="reveal">
            <ScheduleEditor value={draft.restart} onChange={(restart) => set("restart", restart)} />
          </div>
        )}
      </Panel>

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

      {(dirty || saved || error) && (
        <GlassPanel layer tone="raised" className="save-pill" role="status">
          <span className="save-pill-text">
            {error ? (
              <span className="error-text">{error}</span>
            ) : dirty ? (
              running ? "Unsaved changes · they apply on the next start" : "Unsaved changes"
            ) : (
              <span className="row">
                <Icon name="check" size={14} />
                Saved
              </span>
            )}
          </span>
          {dirty && (
            <>
              <GlassButton size="sm" variant="ghost" onClick={discard}>
                Discard
              </GlassButton>
              <GlassButton size="sm" variant="primary" onClick={save}>
                Save changes
              </GlassButton>
            </>
          )}
        </GlassPanel>
      )}
    </div>
  );
}
