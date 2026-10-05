import { useState } from "react";
import type { RestartMode, RestartSchedule } from "../lib/api";
import { GlassButton } from "./glass/GlassButton";
import { GlassInput } from "./glass/GlassInput";
import { Icon } from "./Icon";

interface ScheduleEditorProps {
  value: RestartSchedule;
  onChange: (next: RestartSchedule) => void;
}

const MODES: { mode: RestartMode; label: string; hint: string }[] = [
  { mode: "warn", label: "Warn, then restart", hint: "Countdown in chat at 5 min, 1 min and 10 s; restarts on time." },
  { mode: "postpone", label: "Wait until empty", hint: "If anyone is online, the restart waits until the last player leaves." },
];

/** Daily restart times and what to do when players are online (R6). */
export function ScheduleEditor({ value, onChange }: ScheduleEditorProps) {
  const [time, setTime] = useState("04:00");
  const add = () => {
    if (!/^\d{2}:\d{2}$/.test(time) || value.times.includes(time)) return;
    onChange({ ...value, times: [...value.times, time].sort() });
  };

  return (
    <div className="stack" style={{ gap: 12 }}>
      <div className="row" style={{ flexWrap: "wrap", gap: 6 }} aria-label="Restart times">
        {value.times.length === 0 && <span className="muted">No scheduled restarts.</span>}
        {value.times.map((t) => (
          <span key={t} className="time-chip">
            <Icon name="clock" size={13} />
            <span className="mono">{t}</span>
            <button
              type="button"
              aria-label={`Remove ${t}`}
              onClick={() => onChange({ ...value, times: value.times.filter((x) => x !== t) })}
            >
              <Icon name="kill" size={12} />
            </button>
          </span>
        ))}
      </div>
      <div className="row">
        <GlassInput type="time" aria-label="New restart time" value={time} onChange={(e) => setTime(e.target.value)} style={{ width: 130 }} />
        <GlassButton size="sm" icon={<Icon name="plus" size={13} />} onClick={add}>
          Add time
        </GlassButton>
      </div>
      <div className="stack" role="radiogroup" aria-label="When players are online" style={{ gap: 6 }}>
        {MODES.map((m) => (
          <label key={m.mode} className="radio-row">
            <input type="radio" name="restart-mode" checked={value.mode === m.mode} onChange={() => onChange({ ...value, mode: m.mode })} />
            <span>
              <span className="label">{m.label}</span>
              <span className="hint">{m.hint}</span>
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}
