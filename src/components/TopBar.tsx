import { useEffect, useState, type Ref } from "react";
import { GlassPanel } from "./glass/GlassPanel";
import { Icon } from "./Icon";
import type { Theme } from "../lib/theme";
import { useStore } from "../state/store";

interface TopBarProps {
  /** Receives the element page headers portal into. */
  slotRef: Ref<HTMLDivElement>;
  theme: Theme;
  onToggleTheme: () => void;
}

export function TopBar({ slotRef, theme, onToggleTheme }: TopBarProps) {
  const label = theme === "dark" ? "Switch to light mode" : "Switch to dark mode";
  return (
    <GlassPanel className="topbar">
      <div className="topbar-slot" ref={slotRef} />
      <div className="topbar-divider" aria-hidden="true" />
      <div className="topbar-tools">
        <OnlinePulse />
        <Clock />
        <button type="button" className="icon-button" onClick={onToggleTheme} aria-label={label} title={label}>
          <Icon name={theme === "dark" ? "moon" : "sun"} size={15} />
        </button>
      </div>
    </GlassPanel>
  );
}

/** How many servers are up, with a live dot while any are. */
function OnlinePulse() {
  const online = useStore((s) => s.instances.filter((i) => s.runtime[i.id]?.state === "online").length);
  return (
    <span className="pulse" data-live={online > 0} aria-live="polite">
      <span className="live-dot" aria-hidden="true" />
      {online === 0 ? "No servers online" : online === 1 ? "1 server online" : `${online} servers online`}
    </span>
  );
}

const TICK_MS = 15_000;

function Clock() {
  const [time, setTime] = useState(() => now());
  useEffect(() => {
    const timer = setInterval(() => setTime(now()), TICK_MS);
    return () => clearInterval(timer);
  }, []);
  return <span className="clock">{time}</span>;
}

function now(): string {
  return new Date().toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}
