import { createContext, useContext, type ReactNode } from "react";
import { createPortal } from "react-dom";

/** Where page headers render: the top bar's slot, once it has mounted. */
export const HeaderSlotContext = createContext<HTMLElement | null>(null);

interface PageHeaderProps {
  title: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  actions?: ReactNode;
}

/** The page's title and actions, shown in the window's top bar. */
export function PageHeader({ title, subtitle, leading, actions }: PageHeaderProps) {
  const slot = useContext(HeaderSlotContext);
  const header = (
    <header className="page-header">
      {leading}
      <div className="page-header-text">
        <h1>{title}</h1>
        {subtitle && <div className="subtitle">{subtitle}</div>}
      </div>
      <div className="spacer" />
      {actions && <div className="page-header-actions">{actions}</div>}
    </header>
  );
  // Rendered standalone (tests), there is no top bar: keep the header in place.
  return slot ? createPortal(header, slot) : header;
}
