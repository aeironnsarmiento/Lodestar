import type { ButtonHTMLAttributes, ReactNode } from "react";
import { glassClasses, useEffects } from "./effects";

interface GlassButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "primary" | "danger" | "ghost";
  size?: "sm" | "md";
  icon?: ReactNode;
  /** Renders a square button; pass a `title`/`aria-label` for the accessible name. */
  iconOnly?: boolean;
}

export function GlassButton({
  variant = "default",
  size = "md",
  icon,
  iconOnly,
  className,
  children,
  type = "button",
  ...rest
}: GlassButtonProps) {
  const effects = useEffects();
  const classes = [
    "gbtn",
    variant !== "ghost" ? glassClasses(effects, "control") : "",
    variant !== "default" ? variant : "",
    size === "sm" ? "sm" : "",
    iconOnly ? "icon-only" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type={type} className={classes} {...rest}>
      {icon}
      {!iconOnly && children}
    </button>
  );
}
