import type { ReactNode } from "react";
import { GlassPanel } from "./glass/GlassPanel";

interface PageHeaderProps {
  title: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  actions?: ReactNode;
}

/** Sticky glass header; page content scrolls underneath it. */
export function PageHeader({ title, subtitle, leading, actions }: PageHeaderProps) {
  return (
    <GlassPanel as="header" className="page-header">
      {leading}
      <div>
        <h1>{title}</h1>
        {subtitle && <div className="subtitle">{subtitle}</div>}
      </div>
      <div className="spacer" />
      {actions}
    </GlassPanel>
  );
}
