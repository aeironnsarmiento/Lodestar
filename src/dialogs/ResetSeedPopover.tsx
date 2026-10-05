import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { GlassButton } from "../components/glass/GlassButton";
import { GlassInput, Switch } from "../components/glass/GlassInput";
import { GlassPanel } from "../components/glass/GlassPanel";
import { Icon } from "../components/Icon";

interface ResetButtonProps {
  disabled?: boolean;
  size?: "sm" | "md";
  /** Whether the server is hardcore now; the default for the next world. */
  hardcore?: boolean;
  /** Resets; `seed` is null for a random seed. */
  onReset: (seed: string | null, hardcore: boolean) => void;
}

/**
 * Reset World as a split button: the main part resets right away with a random seed;
 * the arrow opens a small popover with new-world options (seed, hardcore).
 */
export function ResetButton({ disabled, size = "md", hardcore = false, onReset }: ResetButtonProps) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  return (
    <span className="split-button" ref={anchor}>
      <GlassButton
        size={size}
        icon={<Icon name="reset" size={size === "sm" ? 14 : 16} />}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          onReset(null, hardcore);
        }}
      >
        {size === "sm" ? "Reset" : "Reset World"}
      </GlassButton>
      <GlassButton
        size={size}
        iconOnly
        aria-label="New world options"
        aria-expanded={open}
        icon={<span className="chevron" aria-hidden="true" />}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      />
      {open && (
        <ResetSeedPopover
          anchor={anchor.current}
          hardcore={hardcore}
          onClose={() => setOpen(false)}
          onReset={(seed, hc) => {
            setOpen(false);
            onReset(seed, hc);
          }}
        />
      )}
    </span>
  );
}

interface ResetSeedPopoverProps {
  /** The element the popover opens under; clicks on it do not count as outside. */
  anchor: HTMLElement | null;
  hardcore: boolean;
  onClose: () => void;
  onReset: (seed: string | null, hardcore: boolean) => void;
}

const POPOVER_WIDTH = 270;
const EDGE = 12;

/**
 * Where the popover goes: under the anchor, right edges aligned, kept inside the
 * window; above the anchor when there is no room below.
 */
function placement(anchor: HTMLElement | null, height: number): CSSProperties {
  if (!anchor) return { visibility: "hidden" };
  const r = anchor.getBoundingClientRect();
  const left = Math.min(Math.max(r.right - POPOVER_WIDTH, EDGE), window.innerWidth - POPOVER_WIDTH - EDGE);
  const below = r.bottom + 8;
  const top = below + height > window.innerHeight - EDGE && r.top - 8 - height > EDGE ? r.top - 8 - height : below;
  return { position: "fixed", top, left, width: POPOVER_WIDTH };
}

export function ResetSeedPopover({ anchor, hardcore: initialHardcore, onClose, onReset }: ResetSeedPopoverProps) {
  const [seed, setSeed] = useState("");
  const [hardcore, setHardcore] = useState(initialHardcore);
  const ref = useRef<HTMLDivElement>(null);
  const [style, setStyle] = useState<CSSProperties>({ visibility: "hidden" });

  // Rendered on document.body so the page's scroll area and the window frame cannot
  // clip it; it follows its button while the page scrolls or the window resizes.
  useLayoutEffect(() => {
    const place = () => setStyle(placement(anchor, ref.current?.offsetHeight ?? 0));
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [anchor]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (ref.current?.contains(target) || anchor?.contains(target)) return;
      onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [anchor, onClose]);

  return createPortal(
    <div ref={ref} className="popover-anchor" style={style} onClick={(e) => e.stopPropagation()}>
      <GlassPanel layer tone="raised" className="popover" role="dialog" aria-label="New world options">
        <form
          className="stack"
          style={{ gap: 10 }}
          onSubmit={(e) => {
            e.preventDefault();
            onReset(seed.trim() || null, hardcore);
          }}
        >
          <label className="field">
            <span className="label">Seed for the new world</span>
            <GlassInput autoFocus placeholder="Random" value={seed} onChange={(e) => setSeed(e.target.value)} />
          </label>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span className="label">Hardcore</span>
            <Switch label="Hardcore" checked={hardcore} onChange={setHardcore} />
          </div>
          <span className="faint" style={{ fontSize: 12 }}>
            Players are disconnected right away; the old world is kept.
          </span>
          <GlassButton type="submit" variant="primary" icon={<Icon name="reset" size={14} />}>
            Reset World
          </GlassButton>
        </form>
      </GlassPanel>
    </div>,
    document.body,
  );
}
