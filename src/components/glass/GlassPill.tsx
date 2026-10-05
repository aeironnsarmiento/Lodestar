import type { HTMLAttributes, ReactNode } from "react";

interface GlassPillProps extends HTMLAttributes<HTMLSpanElement> {
  children?: ReactNode;
}

/** A small rimmed label (status, counts, addresses). */
export function GlassPill({ className, children, ...rest }: GlassPillProps) {
  return (
    <span className={["gpill", className].filter(Boolean).join(" ")} {...rest}>
      {children}
    </span>
  );
}
