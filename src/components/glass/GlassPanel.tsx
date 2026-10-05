import type { HTMLAttributes, ReactNode } from "react";
import { glassClasses, useEffects } from "./effects";

interface GlassPanelProps extends HTMLAttributes<HTMLElement> {
  as?: "div" | "aside" | "header" | "section" | "nav";
  children?: ReactNode;
}

/** A large glass surface for app chrome: sidebar, sticky headers, dialogs. */
export function GlassPanel({ as: Tag = "div", className, children, ...rest }: GlassPanelProps) {
  const effects = useEffects();
  const classes = [glassClasses(effects, "chrome"), className].filter(Boolean).join(" ");
  return (
    <Tag className={classes} {...rest}>
      {children}
    </Tag>
  );
}
