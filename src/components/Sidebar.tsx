import { GlassPanel } from "./glass/GlassPanel";
import { Icon, type IconName } from "./Icon";
import appIcon from "../assets/lodestar-icon.svg";

export type AppArea = "dashboard" | "playit" | "java" | "settings";

const NAV_ITEMS: { area: AppArea; label: string; icon: IconName }[] = [
  { area: "dashboard", label: "Dashboard", icon: "grid" },
  { area: "playit", label: "playit.gg", icon: "globe" },
  { area: "java", label: "Java runtimes", icon: "cup" },
  { area: "settings", label: "Settings", icon: "gear" },
];

interface SidebarProps {
  active: AppArea;
  onNavigate: (area: AppArea) => void;
}

export function Sidebar({ active, onNavigate }: SidebarProps) {
  return (
    <GlassPanel as="aside" className="sidebar" aria-label="Main navigation">
      <div className="brand">
        <img className="brand-mark" src={appIcon} alt="" />
        <div className="brand-meta">
          <div className="brand-name">Lodestar</div>
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
            title={item.label}
            onClick={() => onNavigate(item.area)}
          >
            <Icon name={item.icon} size={16} />
            <span className="nav-label">{item.label}</span>
          </button>
        ))}
      </nav>
    </GlassPanel>
  );
}
