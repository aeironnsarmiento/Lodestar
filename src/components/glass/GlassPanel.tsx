import type { CSSProperties, HTMLAttributes, ReactNode } from "react";
import { BLUR_DEPTH_CEILING, GlassDepthContext, useEffects, useGlassDepth } from "./effects";

export type GlassTone = "panel" | "raised" | "well";

interface GlassPanelProps extends HTMLAttributes<HTMLElement> {
  as?: "div" | "aside" | "header" | "section" | "nav" | "article";
  tone?: GlassTone;
  /**
   * Starts a new layer floating above everything else (dialogs, popovers): it blurs
   * what is behind it even when it sits inside another glass surface.
   */
  layer?: boolean;
  radius?: string;
  children?: ReactNode;
}

/** A glass surface. The window frame blurs; surfaces nested in it are flat panes. */
export function GlassPanel({
  as: Tag = "div",
  tone = "panel",
  layer = false,
  radius,
  className,
  style,
  children,
  ...rest
}: GlassPanelProps) {
  const { reduceEffects } = useEffects();
  const parentDepth = useGlassDepth();
  const depth = layer ? 1 : parentDepth + 1;
  const blurred = !reduceEffects && depth <= BLUR_DEPTH_CEILING;
  const merged: CSSProperties | undefined = radius ? { borderRadius: radius, ...style } : style;

  return (
    <GlassDepthContext.Provider value={depth}>
      <Tag
        className={className ? `glass ${className}` : "glass"}
        data-blur={blurred ? "on" : "off"}
        data-glass-depth={depth}
        data-tone={tone}
        style={merged}
        {...rest}
      >
        {children}
      </Tag>
    </GlassDepthContext.Provider>
  );
}
