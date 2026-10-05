import type { HTMLAttributes, ReactNode } from "react";
import { glassClasses, useEffects } from "./effects";

interface GlassPillProps extends HTMLAttributes<HTMLSpanElement> {
  children?: ReactNode;
}

/** A small floating label (status, counts, addresses). */
export function GlassPill({ className, children, ...rest }: GlassPillProps) {
  const effects = useEffects();
  const classes = ["gpill", glassClasses(effects, "control"), className].filter(Boolean).join(" ");
  return (
    <span className={classes} {...rest}>
      {children}
    </span>
  );
}
