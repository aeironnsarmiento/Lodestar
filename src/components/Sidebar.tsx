import { GlassPanel } from "./glass/GlassPanel";
import { Icon, type IconName } from "./Icon";

export type AppArea = "dashboard" | "playit" | "java" | "settings";

export const NAV_ITEMS: { area: AppArea; label: string; icon: IconName; hotkey: string }[] = [
  { area: "dashboard", label: "Dashboard", icon: "grid", hotkey: "d" },
  { area: "playit", label: "playit.gg", icon: "globe", hotkey: "p" },
  { area: "java", label: "Java runtimes", icon: "cup", hotkey: "j" },
  { area: "settings", label: "Settings", icon: "gear", hotkey: "s" },
];

interface SidebarProps {
  active: AppArea;
  onNavigate: (area: AppArea) => void;
}

export function Sidebar({ active, onNavigate }: SidebarProps) {
  return (
    <GlassPanel as="aside" className="sidebar" aria-label="Main navigation">
      <div className="brand">
        <div className="brand-mark" aria-hidden="true" />
        <div className="brand-meta">
          <div className="brand-name">Glasscraft</div>
          <div className="brand-handle">server manager</div>
        </div>
      </div>

      <nav className="nav">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.area}
            type="button"
            className={item.area === active ? "nav-item active" : "nav-item"}
            aria-current={item.area === active ? "page" : undefined}
            aria-keyshortcuts={item.hotkey.toUpperCase()}
            title={item.label}
            onClick={() => onNavigate(item.area)}
          >
            <Icon name={item.icon} size={16} />
            <span className="nav-label">{item.label}</span>
            <kbd className="hotkey" aria-hidden="true">
              {item.hotkey}
            </kbd>
          </button>
        ))}
      </nav>

      <p className="sidebar-foot">
        <span>Press a letter to jump to a page.</span>
      </p>
    </GlassPanel>
  );
}
