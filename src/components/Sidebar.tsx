import { GlassPanel } from "./glass/GlassPanel";
import { GlassButton } from "./glass/GlassButton";
import { Icon, type IconName } from "./Icon";
import type { Theme } from "../lib/theme";

export type AppArea = "dashboard" | "playit" | "java" | "settings";

const ITEMS: { area: AppArea; label: string; icon: IconName }[] = [
  { area: "dashboard", label: "Dashboard", icon: "grid" },
  { area: "playit", label: "playit.gg", icon: "globe" },
  { area: "java", label: "Java runtimes", icon: "cup" },
  { area: "settings", label: "Settings", icon: "gear" },
];

interface SidebarProps {
  active: AppArea;
  onNavigate: (area: AppArea) => void;
  theme: Theme;
  onToggleTheme: () => void;
}

export function Sidebar({ active, onNavigate, theme, onToggleTheme }: SidebarProps) {
  return (
    <GlassPanel as="aside" className="sidebar" aria-label="Main navigation">
      <div className="brand">
        <div className="brand-mark" aria-hidden="true" />
        Glasscraft
      </div>
      <nav className="stack" style={{ gap: 2 }}>
        {ITEMS.map((item) => (
          <button
            key={item.area}
            type="button"
            className={item.area === active ? "nav-item active" : "nav-item"}
            aria-current={item.area === active ? "page" : undefined}
            onClick={() => onNavigate(item.area)}
          >
            <Icon name={item.icon} />
            {item.label}
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <GlassButton
          size="sm"
          icon={<Icon name={theme === "dark" ? "sun" : "moon"} size={16} />}
          onClick={onToggleTheme}
          aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        >
          {theme === "dark" ? "Light mode" : "Dark mode"}
        </GlassButton>
      </div>
    </GlassPanel>
  );
}
