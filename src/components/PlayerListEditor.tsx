import { useState } from "react";
import { GlassButton } from "./glass/GlassButton";
import { GlassInput } from "./glass/GlassInput";
import { Icon } from "./Icon";

const PLAYER_NAME = /^[A-Za-z0-9_]{1,16}$/;

interface PlayerListEditorProps {
  label: string;
  names: string[];
  empty: string;
  onChange: (names: string[]) => void;
}

/** Chips of Minecraft names with an add field; names are checked and de-duplicated. */
export function PlayerListEditor({ label, names, empty, onChange }: PlayerListEditorProps) {
  const [name, setName] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  const add = () => {
    const n = name.trim();
    if (!PLAYER_NAME.test(n)) {
      setProblem("Names are 1–16 letters, numbers or underscores.");
      return;
    }
    setProblem(null);
    setName("");
    if (!names.some((x) => x.toLowerCase() === n.toLowerCase())) onChange([...names, n]);
  };

  return (
    <div className="player-list-editor stack">
      <div className="label">{label}</div>
      <div className="row" style={{ flexWrap: "wrap", gap: 6 }} aria-label={label}>
        {names.length === 0 && <span className="muted">{empty}</span>}
        {names.map((n) => (
          <span key={n} className="time-chip">
            <span>{n}</span>
            <button type="button" aria-label={`Remove ${n}`} onClick={() => onChange(names.filter((x) => x !== n))}>
              <Icon name="kill" size={12} />
            </button>
          </span>
        ))}
      </div>
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <GlassInput
          aria-label={`Add to ${label.toLowerCase()}`}
          placeholder="Minecraft name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          style={{ width: 200 }}
        />
        <GlassButton type="submit" size="sm" icon={<Icon name="plus" size={13} />} disabled={!name.trim()}>
          Add
        </GlassButton>
      </form>
      {problem && <span className="error-text hint">{problem}</span>}
    </div>
  );
}
