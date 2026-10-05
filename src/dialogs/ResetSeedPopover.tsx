import { useEffect, useRef, useState } from "react";
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
  return (
    <span className="split-button">
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
  hardcore: boolean;
  onClose: () => void;
  onReset: (seed: string | null, hardcore: boolean) => void;
}

export function ResetSeedPopover({ hardcore: initialHardcore, onClose, onReset }: ResetSeedPopoverProps) {
  const [seed, setSeed] = useState("");
  const [hardcore, setHardcore] = useState(initialHardcore);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <div ref={ref} onClick={(e) => e.stopPropagation()}>
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
    </div>
  );
}
