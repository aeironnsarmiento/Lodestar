import { useEffect, useMemo, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { GlassButton } from "../components/glass/GlassButton";
import { Switch } from "../components/glass/GlassInput";
import { Icon } from "../components/Icon";
import {
  api,
  CURSEFORGE_ENABLED,
  loadersFor,
  SOURCE_LABELS,
  type AddonEntry,
  type AddonSource,
  type InstallResult,
  type Instance,
  type ProjectKind,
  type ProjectVersion,
  type SearchHit,
} from "../lib/api";
import { errorMessage } from "../state/store";
import { Dialog } from "./Dialog";

const KIND_LABELS: Record<ProjectKind, string> = { mod: "mods", plugin: "plugins", modpack: "modpacks" };

interface ProjectBrowserProps {
  kind: ProjectKind;
  onClose: () => void;
  /** Add-on mode: the server to install into and what it already has. */
  instance?: Instance;
  installed?: AddonEntry[];
  onInstalled?: (result: InstallResult) => void;
  /** Modpack mode: called with the chosen pack version. */
  onPickModpack?: (hit: SearchHit, version: ProjectVersion) => void;
  /** Opens app settings (for the CurseForge key). */
  onOpenSettings?: () => void;
}

/** Compact download counts: 1.2M, 34k. */
export function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}k`;
  return String(n);
}

export function ProjectIcon({ url, title, size = 40 }: { url: string | null; title: string; size?: number }) {
  const [broken, setBroken] = useState(false);
  return (
    <span className="project-icon" style={{ width: size, height: size }} aria-hidden="true">
      {url && !broken ? <img src={url} alt="" onError={() => setBroken(true)} /> : title.slice(0, 1).toUpperCase()}
    </span>
  );
}

function open(url: string) {
  if (url) openUrl(url).catch(() => {});
}

/**
 * Browse Modrinth (and CurseForge, once enabled). For a server it shows only what that server can
 * run (its loader and Minecraft version) unless "Compatible only" is turned off; for
 * modpacks it lists every pack and hands the chosen version back.
 */
export function ProjectBrowser({ kind, onClose, instance, installed = [], onInstalled, onPickModpack, onOpenSettings }: ProjectBrowserProps) {
  const [source, setSource] = useState<AddonSource>("modrinth");
  const [text, setText] = useState("");
  const [query, setQuery] = useState("");
  const [compatible, setCompatible] = useState(true);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<SearchHit | null>(null);
  const [versions, setVersions] = useState<ProjectVersion[] | null>(null);
  const [versionsError, setVersionsError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<InstallResult | null>(null);
  const [justInstalled, setJustInstalled] = useState<Set<string>>(new Set());
  const request = useRef(0);

  const loaders = useMemo(() => (instance && compatible ? loadersFor(instance.serverType) : []), [instance, compatible]);
  const gameVersion = instance && compatible ? instance.mcVersion : null;

  // Debounce typing.
  useEffect(() => {
    const t = setTimeout(() => setQuery(text), 350);
    return () => clearTimeout(t);
  }, [text]);

  const load = async (offset: number) => {
    const ticket = ++request.current;
    setLoading(true);
    setError(null);
    try {
      const page = await api.searchProjects(source, kind, query, gameVersion, loaders, offset);
      if (ticket !== request.current) return;
      setHits((h) => (offset === 0 ? page.hits : [...h, ...page.hits]));
      setTotal(page.total);
      if (offset === 0) setSelected(page.hits[0] ?? null);
    } catch (e) {
      if (ticket !== request.current) return;
      setError(errorMessage(e));
      if (offset === 0) {
        setHits([]);
        setSelected(null);
      }
    } finally {
      if (ticket === request.current) setLoading(false);
    }
  };

  useEffect(() => {
    load(0);
  }, [source, query, compatible]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!selected) {
      setVersions(null);
      return;
    }
    let live = true;
    setVersions(null);
    setVersionsError(null);
    api
      .projectVersions(selected.source, selected.projectId, loaders, gameVersion)
      .then((v) => live && setVersions(v))
      .catch((e) => live && setVersionsError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [selected, loaders, gameVersion]);

  const isInstalled = (hit: SearchHit) =>
    justInstalled.has(`${hit.source}:${hit.projectId}`) ||
    installed.some((e) => e.source === hit.source && e.projectId === hit.projectId);

  const install = async (hit: SearchHit, version: ProjectVersion | null) => {
    if (!instance) return;
    setBusy(version?.versionId ?? "latest");
    setResult(null);
    try {
      const r = await api.installProject(instance.id, hit.source, hit.projectId, version?.versionId ?? null);
      setResult(r);
      setJustInstalled((s) => new Set(s).add(`${hit.source}:${hit.projectId}`));
      onInstalled?.(r);
    } catch (e) {
      setResult({ installed: [], blocked: [], notes: [errorMessage(e)] });
    } finally {
      setBusy(null);
    }
  };

  const needsKey = source === "curseforge" && error?.includes("API key");
  const what = KIND_LABELS[kind];

  return (
    <Dialog title={onPickModpack ? "Choose a modpack" : `Browse ${what}`} onClose={onClose} width={1040} className="browser-dialog">
      <div className="browser-toolbar">
        {CURSEFORGE_ENABLED && (
        <div className="segmented browser-sources" role="radiogroup" aria-label="Site">
          {(["modrinth", "curseforge"] as AddonSource[]).map((s) => (
            <button
              key={s}
              type="button"
              role="radio"
              aria-checked={source === s}
              className={source === s ? "segment active" : "segment"}
              onClick={() => setSource(s)}
            >
              {SOURCE_LABELS[s]}
            </button>
          ))}
        </div>
        )}
        <div className="console-search browser-search">
          <Icon name="search" size={15} />
          <input
            type="search"
            aria-label={`Search ${what}`}
            placeholder={`Search ${what}`}
            value={text}
            autoFocus
            onChange={(e) => setText(e.target.value)}
          />
        </div>
        {instance && (
          <label className="row browser-compat">
            <Switch label="Compatible only" checked={compatible} onChange={setCompatible} />
            <span className="faint">
              {compatible ? `${loadersFor(instance.serverType)[0] ?? ""} ${instance.mcVersion}` : "Any version"}
            </span>
          </label>
        )}
      </div>

      <div className="browser-body">
        <div className="browser-results" role="listbox" aria-label="Results">
          {error && (
            <div className="empty">
              <span className="error-text">{error}</span>
              {needsKey && onOpenSettings && (
                <GlassButton size="sm" onClick={onOpenSettings}>
                  Open Settings
                </GlassButton>
              )}
            </div>
          )}
          {!error && hits.length === 0 && !loading && <div className="empty">No {what} found.</div>}
          {hits.map((hit) => (
            <button
              key={`${hit.source}:${hit.projectId}`}
              type="button"
              role="option"
              aria-selected={selected?.projectId === hit.projectId}
              className="browser-hit"
              onClick={() => setSelected(hit)}
            >
              <ProjectIcon url={hit.iconUrl} title={hit.title} />
              <span className="browser-hit-text">
                <span className="browser-hit-title">
                  {hit.title}
                  {isInstalled(hit) && <span className="tag">Installed</span>}
                </span>
                <span className="browser-hit-desc">{hit.description}</span>
                <span className="browser-hit-meta mono">
                  {hit.author && `by ${hit.author} · `}
                  {formatCount(hit.downloads)} downloads
                </span>
              </span>
            </button>
          ))}
          {loading && <div className="empty faint">Searching…</div>}
          {!loading && hits.length > 0 && hits.length < total && (
            <div className="row" style={{ justifyContent: "center", padding: 8 }}>
              <GlassButton size="sm" onClick={() => load(hits.length)}>
                Load more
              </GlassButton>
            </div>
          )}
        </div>

        <div className="browser-detail">
          {!selected ? (
            <div className="empty faint">Pick a project to see its versions.</div>
          ) : (
            <>
              <div className="browser-detail-head">
                <ProjectIcon url={selected.iconUrl} title={selected.title} size={56} />
                <div style={{ minWidth: 0 }}>
                  <h3>{selected.title}</h3>
                  <div className="faint mono" style={{ fontSize: "0.72rem" }}>
                    {selected.author && `by ${selected.author} · `}
                    {formatCount(selected.downloads)} downloads · {SOURCE_LABELS[selected.source]}
                  </div>
                </div>
              </div>
              <p className="muted browser-detail-desc">{selected.description}</p>
              <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                {instance && (
                  <GlassButton
                    variant="primary"
                    icon={<Icon name="plus" size={14} />}
                    disabled={busy !== null || versions?.length === 0}
                    onClick={() => install(selected, null)}
                  >
                    {busy === "latest" ? "Installing…" : isInstalled(selected) ? "Reinstall latest" : "Install latest"}
                  </GlassButton>
                )}
                <GlassButton icon={<Icon name="external" size={14} />} onClick={() => open(selected.pageUrl)}>
                  Open page
                </GlassButton>
              </div>

              {result && (
                <div className="browser-result" role="status">
                  {result.installed.length > 0 && (
                    <p>
                      <Icon name="check" size={14} /> Installed {result.installed.join(", ")}. It loads on the next start.
                    </p>
                  )}
                  {result.blocked.map((b) => (
                    <p key={b.fileName}>
                      The author of {b.title} does not allow apps to download it.{" "}
                      <a
                        href={b.pageUrl}
                        onClick={(e) => {
                          e.preventDefault();
                          open(b.pageUrl);
                        }}
                      >
                        Download it by hand
                      </a>{" "}
                      and drop {b.fileName} into the Mods tab.
                    </p>
                  ))}
                  {result.notes.map((n) => (
                    <p key={n} className="error-text">
                      {n}
                    </p>
                  ))}
                </div>
              )}

              <h4 className="section-title" style={{ marginTop: 6 }}>
                Versions
              </h4>
              <div className="browser-versions">
                {versionsError && <p className="error-text">{versionsError}</p>}
                {!versions && !versionsError && <p className="faint">Loading versions…</p>}
                {versions?.length === 0 && (
                  <p className="muted">
                    No versions{instance && compatible ? ` for ${loadersFor(instance.serverType)[0]} ${instance.mcVersion}` : ""}.
                  </p>
                )}
                {versions?.slice(0, 40).map((v) => (
                  <div key={v.versionId} className="browser-version">
                    <div style={{ minWidth: 0 }}>
                      <div className="browser-version-name">
                        {v.versionNumber}
                        {v.channel !== "release" && <span className="tag">{v.channel}</span>}
                      </div>
                      <div className="faint mono browser-version-meta">
                        {v.loaders.join(", ")} · {v.gameVersions.slice(0, 4).join(", ")}
                        {v.gameVersions.length > 4 ? "…" : ""}
                      </div>
                    </div>
                    {onPickModpack ? (
                      <GlassButton size="sm" variant="primary" onClick={() => onPickModpack(selected, v)}>
                        Use this
                      </GlassButton>
                    ) : (
                      <GlassButton size="sm" disabled={busy !== null} onClick={() => install(selected, v)}>
                        {busy === v.versionId ? "Installing…" : "Install"}
                      </GlassButton>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
