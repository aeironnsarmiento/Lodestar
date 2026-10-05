import { createContext, useContext } from "react";

export interface GlassEffects {
  /** "Reduce effects": no blur, no refraction, flat translucent fills. */
  reduceEffects: boolean;
  /** Result of the refraction feature check. */
  refraction: boolean;
}

export const EffectsContext = createContext<GlassEffects>({ reduceEffects: false, refraction: false });

export function useEffects(): GlassEffects {
  return useContext(EffectsContext);
}

/**
 * Class list for a glass surface. Chrome surfaces blur; small floating controls may
 * also refract. Reduce effects drops both and uses a flat fill.
 */
export function glassClasses(effects: GlassEffects, kind: "chrome" | "control"): string {
  if (effects.reduceEffects) return "glass glass-flat";
  if (kind === "control") {
    return effects.refraction ? "glass glass-control glass-blur glass-refract" : "glass glass-control glass-blur";
  }
  return "glass glass-blur";
}

/** The SVG filter referenced by `.glass-refract`. Rendered once at the app root. */
export function RefractionFilter() {
  return (
    <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
      <filter id="glass-refraction" x="0%" y="0%" width="100%" height="100%" colorInterpolationFilters="sRGB">
        <feTurbulence type="fractalNoise" baseFrequency="0.008 0.012" numOctaves="2" seed="7" result="noise" />
        <feGaussianBlur in="noise" stdDeviation="2" result="soft" />
        <feDisplacementMap in="SourceGraphic" in2="soft" scale="18" xChannelSelector="R" yChannelSelector="G" />
      </filter>
    </svg>
  );
}
