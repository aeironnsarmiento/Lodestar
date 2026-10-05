import { useState } from "react";
import { open as openFiles } from "@tauri-apps/plugin-dialog";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassInput } from "../components/glass/GlassInput";
import { Icon } from "../components/Icon";
import {
  api,
  CURSEFORGE_ENABLED,
  SERVER_TYPE_LABELS,
  SOURCE_LABELS,
  type Instance,
  type ModpackRef,
  type ProjectVersion,
  type SearchHit,
  type ServerType,
} from "../lib/api";
import { errorMessage } from "../state/store";
import { ProjectBrowser, ProjectIcon } from "./ProjectBrowser";

interface PickedPack {
  ref: Partial<ModpackRef>;
  serverType: ServerType;
  mcVersion: string;
  loaderVersion: string | null;
}

/** The loader a pack version runs on, in the order Lodestar prefers. */
export function serverTypeForLoaders(loaders: string[]): ServerType | null {
  if (loaders.includes("fabric")) return "fabric";
  if (loaders.includes("neoforge")) return "neoforge";
  if (loaders.includes("forge")) return "forge";
  return null;
}

interface ModpackServerFormProps {
  onClose: () => void;
  onCreated: (instance: Instance) => void;
  onOpenSettings?: () => void;
}

/** New server → From a modpack: pick a pack online or import a file, then create. */
export function ModpackServerForm({ onClose, onCreated, onOpenSettings }: ModpackServerFormProps) {
  const [picked, setPicked] = useState<PickedPack | null>(null);
  const [name, setName] = useState("");
  const [browsing, setBrowsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const fromBrowser = (hit: SearchHit, v: ProjectVersion) => {
    setBrowsing(false);
    const serverType = serverTypeForLoaders(v.loaders);
    if (!serverType) {
      setError(`${hit.title} ${v.versionNumber} runs on ${v.loaders.join(", ") || "an unknown loader"}, which Lodestar cannot host yet.`);
      return;
    }
    setError(null);
    setName(hit.title);
    setPicked({
      serverType,
      mcVersion: v.gameVersions[0] ?? "",
      loaderVersion: null,
      ref: {
        source: hit.source,
        projectId: hit.projectId,
        versionId: v.versionId,
        title: hit.title,
        versionNumber: v.versionNumber,
        iconUrl: hit.iconUrl,
        pageUrl: hit.pageUrl,
      },
    });
  };

  const importFile = async () => {
    setError(null);
    try {
      const path = await openFiles({ title: "Import a modpack", filters: [{ name: "Modpacks", extensions: CURSEFORGE_ENABLED ? ["mrpack", "zip"] : ["mrpack"] }] });
      if (!path || Array.isArray(path)) return;
      const info = await api.inspectModpackFile(path);
      setName(info.name);
      setPicked({
        serverType: info.serverType,
        mcVersion: info.mcVersion,
        loaderVersion: info.loaderVersion,
        ref: { source: null, title: info.name, versionNumber: info.version || null, file: path },
      });
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const create = async () => {
    if (!picked || !name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const inst = await api.createInstance({
        name: name.trim(),
        serverType: picked.serverType,
        mcVersion: picked.mcVersion,
        loaderVersion: picked.loaderVersion,
        modpack: picked.ref,
        gameMode: "survival",
        difficulty: "easy",
      });
      onCreated(inst);
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  return (
    <div className="stack">
      {!picked ? (
        <div className="modpack-choices">
          <button type="button" className="modpack-choice" onClick={() => setBrowsing(true)}>
            <Icon name="search" size={22} />
            <span className="label">Browse modpacks</span>
            <span className="hint">{CURSEFORGE_ENABLED ? "Modrinth and CurseForge" : "From Modrinth"}</span>
          </button>
          <button type="button" className="modpack-choice" onClick={importFile}>
            <Icon name="folder" size={22} />
            <span className="label">Import a file</span>
            <span className="hint">{CURSEFORGE_ENABLED ? ".mrpack or CurseForge .zip" : "A Modrinth .mrpack file"}</span>
          </button>
        </div>
      ) : (
        <>
          <div className="surface modpack-card picked">
            <ProjectIcon url={picked.ref.iconUrl ?? null} title={picked.ref.title ?? "?"} size={48} />
            <div style={{ minWidth: 0, flex: 1 }}>
              <div className="modpack-title">{picked.ref.title}</div>
              <div className="faint mono" style={{ fontSize: "0.72rem" }}>
                {picked.ref.versionNumber ?? ""} · {SERVER_TYPE_LABELS[picked.serverType]} {picked.mcVersion}
                {picked.ref.source ? ` · ${SOURCE_LABELS[picked.ref.source]}` : " · from a file"}
              </div>
            </div>
            <GlassButton size="sm" onClick={() => setPicked(null)}>
              Change
            </GlassButton>
          </div>
          <div className="field">
            <label htmlFor="mp-name">Name</label>
            <GlassInput id="mp-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          <p className="faint" style={{ fontSize: "0.78rem", margin: 0 }}>
            Lodestar installs the pack's server files, loader and settings. Big packs may want more memory — set it in the
            server's Settings tab.
          </p>
        </>
      )}

      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      <div className="dialog-footer">
        <GlassButton onClick={onClose}>Cancel</GlassButton>
        <GlassButton variant="primary" disabled={!picked || !name.trim() || busy} onClick={create}>
          Create server
        </GlassButton>
      </div>

      {browsing && (
        <ProjectBrowser kind="modpack" onClose={() => setBrowsing(false)} onPickModpack={fromBrowser} onOpenSettings={onOpenSettings} />
      )}
    </div>
  );
}
