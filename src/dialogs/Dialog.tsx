import { useEffect, useId, type ReactNode } from "react";
import { GlassPanel } from "../components/glass/GlassPanel";

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}

/** A modal glass sheet. Escape and the backdrop close it. */
export function Dialog({ title, onClose, children, footer, width = 520 }: DialogProps) {
  const titleId = useId();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <GlassPanel className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} style={{ width }}>
        <h2 id={titleId} className="dialog-title">
          {title}
        </h2>
        <div className="dialog-body">{children}</div>
        {footer && <div className="dialog-footer">{footer}</div>}
      </GlassPanel>
    </div>
  );
}
