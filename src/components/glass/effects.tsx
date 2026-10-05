import { createContext, useContext } from "react";

export interface GlassEffects {
  /** "Reduce effects": no backdrop blur anywhere, and no motion. */
  reduceEffects: boolean;
}

export const EffectsContext = createContext<GlassEffects>({ reduceEffects: false });

export function useEffects(): GlassEffects {
  return useContext(EffectsContext);
}

/**
 * Only surfaces at depth 1 blur. A surface nested inside another one renders as a
 * flat translucent pane, so a layer never stacks backdrop filters.
 */
export const BLUR_DEPTH_CEILING = 1;

export const GlassDepthContext = createContext(0);

export function useGlassDepth(): number {
  return useContext(GlassDepthContext);
}
