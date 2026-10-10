import { useEffect, useState } from "react";
import { GlassSelect } from "./glass/GlassInput";
import { api, SERVER_TYPE_LABELS, type LoaderChoices, type LoaderOption, type ServerType } from "../lib/api";
import { errorMessage } from "../state/store";

function optionLabel(v: LoaderOption, installed: string | null): string {
  const notes = [v.tag, v.id === installed && "installed", !v.compatible && "a mod needs another version"];
  const extra = notes.filter(Boolean).join(", ");
  return extra ? `${v.id} (${extra})` : v.id;
}

/**
 * Builds on offer for a type and Minecraft version; `id` checks them against that
 * server's mods. A change in `refresh` (a reinstall finishing, say) fetches again.
 */
export function useLoaderChoices(serverType: ServerType, mcVersion: string, id: string | null, refresh: unknown = 0) {
  const [choices, setChoices] = useState<LoaderChoices | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setChoices(null);
    setError(null);
    if (!mcVersion) return;
    let cancelled = false;
    api
      .loaderChoices(serverType, mcVersion, id)
      .then((c) => !cancelled && setChoices(c))
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [serverType, mcVersion, id, refresh]);
  return { choices, error };
}

interface LoaderVersionSelectProps {
  id?: string;
  serverType: ServerType;
  choices: LoaderChoices | null;
  error: string | null;
  /** `null` = Automatic. */
  value: string | null;
  onChange: (value: string | null) => void;
  disabled?: boolean;
}

/** The loader build picker: "Automatic" plus every build, newest first. */
export function LoaderVersionSelect({ id, serverType, choices, error, value, onChange, disabled }: LoaderVersionSelectProps) {
  const loader = SERVER_TYPE_LABELS[serverType];
  const effective = value ?? choices?.automatic ?? null;
  const picked = choices?.versions.find((v) => v.id === effective);
  const refusing = picked && choices ? picked.rejectedBy.map((i) => choices.requirements[i]) : [];

  return (
    <div className="stack" style={{ gap: 4 }}>
      <GlassSelect
        id={id}
        aria-label={`${loader} version`}
        value={value ?? ""}
        disabled={disabled || !choices}
        onChange={(e) => onChange(e.target.value || null)}
      >
        {!choices && !error && <option value="">Loading versions…</option>}
        {choices && <option value="">{choices.automatic ? `Automatic (${choices.automatic})` : "Automatic"}</option>}
        {choices?.versions.map((v) => (
          <option key={v.id} value={v.id}>
            {optionLabel(v, choices.installed)}
          </option>
        ))}
        {/* A pinned build the list no longer has (a modpack's, say) stays selectable. */}
        {choices && value && !choices.versions.some((v) => v.id === value) && <option value={value}>{value}</option>}
      </GlassSelect>
      {error && (
        <span className="error-text" style={{ fontSize: 12.5 }}>
          Could not load {loader} versions: {error}
        </span>
      )}
      {refusing.map((r) => (
        <span key={r.fileName + r.range} className="warn-text" style={{ fontSize: 12.5 }}>
          {r.modName} needs {loader} {r.summary}.
        </span>
      ))}
    </div>
  );
}
