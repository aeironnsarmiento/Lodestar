import { useEffect, useMemo, useState } from "react";
import { open as openFiles } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { GlassButton } from "../../components/glass/GlassButton";
import { Switch } from "../../components/glass/GlassInput";
import { Icon } from "../../components/Icon";
import { ProjectBrowser, ProjectIcon } from "../../dialogs/ProjectBrowser";
import { ModpackVersionsDialog } from "../../dialogs/ModpackVersionsDialog";
import { api, inTauri, SOURCE_LABELS, type AddonEntry, type AddonUpdate, type Instance } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { errorMessage } from "../../state/store";

interface ModsTabProps {
  instance: Instance;
  /** The server process is up; its files are locked until it stops. */
  running: boolean;
  onOpenSettings?: () => void;
}

/**
 * The server's mods (or plugins) folder: drop `.jar` files anywhere on the window,
 * browse Modrinth, turn files on and off, update and remove them.
 */
export function ModsTab({ instance, running, onOpenSettings }: ModsTabProps) {
  const plugins = instance.serverType === "paper";
  const noun = plugins ? "plugin" : "mod";
  const Noun = plugins ? "Plugins" : "Mods";
  const [entries, setEntries] = useState<AddonEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [browsing, setBrowsing] = useState(false);
  const [changingPack, setChangingPack] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [updates, setUpdates] = useState<AddonUpdate[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [working, setWorking] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setEntries(await api.listAddons(instance.id));
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  // List right away, then look up hand-added files on Modrinth in the background.
  useEffect(() => {
    let live = true;
    api
      .listAddons(instance.id)
      .then((list) => {
        if (!live) return;
        setEntries(list);
        if (list.some((e) => !e.source && !e.unidentified)) {
          api.identifyAddons(instance.id).then((l) => live && setEntries(l)).catch(() => {});
        }
      })
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [instance.id]);

  const importPaths = async (paths: string[]) => {
    if (paths.length === 0) return;
    setError(null);
    try {
      const r = await api.importAddons(instance.id, paths);
      const parts: string[] = [];
      if (r.added.length) parts.push(`Added ${r.added.length} ${noun}${r.added.length === 1 ? "" : "s"}.`);
      parts.push(...r.skipped);
      setNotice(parts.join(" "));
      await refresh();
      if (r.added.length) api.identifyAddons(instance.id).then(setEntries).catch(() => {});
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  // Drag and drop from Explorer: Tauri hands over real file paths.
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") setDragging(true);
        else if (p.type === "leave") setDragging(false);
        else if (p.type === "drop") {
          setDragging(false);
          importPaths(p.paths);
        }
      })
      .then((u) => (unlisten = u))
      .catch(() => {});
    return () => unlisten?.();
  }, [instance.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const pickFiles = async () => {
    try {
      const picked = await openFiles({ multiple: true, title: `Add ${noun}s`, filters: [{ name: Noun, extensions: ["jar"] }] });
      if (picked) importPaths(Array.isArray(picked) ? picked : [picked]);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const act = async (key: string, action: () => Promise<unknown>) => {
    setWorking(key);
    setError(null);
    try {
      await action();
      await refresh();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setWorking(null);
    }
  };

  const checkUpdates = async () => {
    setChecking(true);
    setError(null);
    try {
      const u = await api.checkAddonUpdates(instance.id);
      setUpdates(u);
      setNotice(u.length ? null : `Everything is up to date.`);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setChecking(false);
    }
  };

  const update = (u: AddonUpdate) =>
    act(u.fileName, async () => {
      await api.updateAddon(instance.id, u.fileName, u.versionId);
      setUpdates((list) => list?.filter((x) => x.fileName !== u.fileName) ?? null);
    });

  const updateAll = async () => {
    for (const u of updates ?? []) await update(u);
  };

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!entries) return [];
    if (!q) return entries;
    return entries.filter((e) => (e.title ?? "").toLowerCase().includes(q) || e.fileName.toLowerCase().includes(q));
  }, [entries, filter]);

  const disabledCount = entries?.filter((e) => !e.enabled).length ?? 0;
  const pack = instance.modpack;

  return (
    <div className="stack mods-tab" style={{ gap: 16 }}>
      {pack && (
        <section className="surface panel modpack-card" aria-label="Modpack">
          <ProjectIcon url={pack.iconUrl} title={pack.title} size={52} />
          <div style={{ minWidth: 0, flex: 1 }}>
            <div className="section-title" style={{ margin: 0 }}>
              Modpack
            </div>
            <div className="modpack-title">{pack.title}</div>
            <div className="faint mono" style={{ fontSize: "0.72rem" }}>
              {pack.versionNumber ?? "Unknown version"}
              {pack.source ? ` · ${SOURCE_LABELS[pack.source]}` : " · from a file"}
              {!pack.installed && " · installing…"}
            </div>
          </div>
          {pack.source && pack.projectId && (
            <GlassButton size="sm" disabled={running} title={running ? "Stop the server first" : undefined} onClick={() => setChangingPack(true)}>
              Change version
            </GlassButton>
          )}
          {pack.pageUrl && (
            <GlassButton size="sm" iconOnly aria-label="Open the modpack's page" icon={<Icon name="external" size={14} />} onClick={() => openUrl(pack.pageUrl!).catch(() => {})} />
          )}
          {pack.missing.length > 0 && (
            <div className="modpack-missing">
              <p className="error-text">These files must be downloaded by hand and dropped here:</p>
              <ul>
                {pack.missing.map((m) => (
                  <li key={m.fileName}>
                    <a
                      href={m.pageUrl}
                      onClick={(e) => {
                        e.preventDefault();
                        if (m.pageUrl) openUrl(m.pageUrl).catch(() => {});
                      }}
                    >
                      {m.title}
                    </a>{" "}
                    <span className="mono faint">{m.fileName}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}

      <div className="mods-toolbar">
        <GlassButton variant="primary" icon={<Icon name="search" size={14} />} onClick={() => setBrowsing(true)}>
          Browse {noun}s
        </GlassButton>
        <GlassButton icon={<Icon name="plus" size={14} />} onClick={pickFiles}>
          Add files…
        </GlassButton>
        <GlassButton icon={<Icon name="restart" size={14} />} disabled={checking || !entries?.length} onClick={checkUpdates}>
          {checking ? "Checking…" : "Check for updates"}
        </GlassButton>
        <GlassButton iconOnly aria-label={`Open the ${noun}s folder`} title="Open folder" icon={<Icon name="folder" size={15} />} onClick={() => api.openAddonsFolder(instance.id).catch(() => {})} />
        <div className="spacer" style={{ flex: 1 }} />
        <div className="console-search">
          <Icon name="search" size={15} />
          <input type="search" aria-label={`Filter ${noun}s`} placeholder="Filter" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
      </div>

      {running && <p className="muted mods-note">The server is running: new {noun}s load on the next start, and files can be turned off, updated or removed once it stops.</p>}
      {notice && (
        <p className="muted mods-note" role="status">
          {notice}
        </p>
      )}
      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      {updates && updates.length > 0 && (
        <div className="surface update-banner" role="status">
          <span>
            {updates.length} update{updates.length === 1 ? "" : "s"} available
          </span>
          <GlassButton size="sm" variant="primary" disabled={running || working !== null} onClick={updateAll}>
            Update all
          </GlassButton>
        </div>
      )}

      <section className={dragging ? "surface panel mods-list dragging" : "surface panel mods-list"} aria-label={Noun}>
        <header className="panel-head">
          <h2 className="section-title">
            {Noun}
            {entries ? ` · ${entries.length}` : ""}
            {disabledCount ? ` (${disabledCount} off)` : ""}
          </h2>
          <span className="panel-note">Drop .jar files anywhere to add them</span>
        </header>

        {entries === null && !error && <div className="empty faint">Loading…</div>}
        {entries?.length === 0 && (
          <div className="empty drop-hint">
            <Icon name="folder" size={26} />
            <span>No {noun}s yet. Drop .jar files here, or browse Modrinth.</span>
          </div>
        )}
        {entries && entries.length > 0 && shown.length === 0 && <div className="empty faint">Nothing matches “{filter}”.</div>}

        <ul className="mod-rows">
          {shown.map((e) => {
            const upd = updates?.find((u) => u.fileName === e.fileName);
            const title = e.title ?? e.fileName.replace(/\.jar$/i, "");
            return (
              <li key={e.fileName} className={e.enabled ? "mod-row" : "mod-row off"}>
                <ProjectIcon url={e.iconUrl} title={title} size={36} />
                <div className="mod-row-text">
                  <div className="mod-row-title">
                    {title}
                    {e.fromModpack && <span className="tag">Modpack</span>}
                    {e.source ? <span className="tag quiet">{SOURCE_LABELS[e.source]}</span> : <span className="tag quiet">Added by hand</span>}
                  </div>
                  <div className="mod-row-meta mono">
                    {e.versionNumber ? `${e.versionNumber} · ` : ""}
                    {e.fileName} · {formatBytes(e.size)}
                  </div>
                </div>
                {upd && (
                  <GlassButton size="sm" variant="primary" disabled={running || working !== null} onClick={() => update(upd)}>
                    {working === e.fileName ? "Updating…" : `Update to ${upd.versionNumber}`}
                  </GlassButton>
                )}
                {e.pageUrl && (
                  <GlassButton
                    size="sm"
                    iconOnly
                    variant="ghost"
                    aria-label={`Open ${title}'s page`}
                    icon={<Icon name="external" size={14} />}
                    onClick={() => openUrl(e.pageUrl!).catch(() => {})}
                  />
                )}
                <Switch
                  label={`${title} enabled`}
                  checked={e.enabled}
                  disabled={running || working !== null}
                  onChange={(on) => act(e.fileName, () => api.setAddonEnabled(instance.id, e.fileName, on))}
                />
                {confirmRemove === e.fileName ? (
                  <span className="row">
                    <GlassButton size="sm" onClick={() => setConfirmRemove(null)}>
                      Keep
                    </GlassButton>
                    <GlassButton
                      size="sm"
                      variant="danger"
                      onClick={() => {
                        setConfirmRemove(null);
                        act(e.fileName, () => api.removeAddon(instance.id, e.fileName));
                      }}
                    >
                      Remove
                    </GlassButton>
                  </span>
                ) : (
                  <GlassButton
                    size="sm"
                    iconOnly
                    variant="danger"
                    aria-label={`Remove ${title}`}
                    title={running ? "Stop the server to remove files" : "Remove"}
                    disabled={running || working !== null}
                    icon={<Icon name="trash" size={14} />}
                    onClick={() => setConfirmRemove(e.fileName)}
                  />
                )}
              </li>
            );
          })}
        </ul>
        {dragging && (
          <div className="drop-overlay" aria-hidden="true">
            <Icon name="plus" size={28} />
            Drop to add {noun}s
          </div>
        )}
      </section>

      {browsing && (
        <ProjectBrowser
          kind={plugins ? "plugin" : "mod"}
          instance={instance}
          installed={entries ?? []}
          onOpenSettings={onOpenSettings}
          onInstalled={() => refresh()}
          onClose={() => setBrowsing(false)}
        />
      )}
      {changingPack && pack && <ModpackVersionsDialog instance={instance} onClose={() => setChangingPack(false)} />}
    </div>
  );
}
