import type { ButtonHTMLAttributes, ReactNode } from "react";

interface GlassButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "primary" | "danger" | "ghost";
  size?: "sm" | "md";
  icon?: ReactNode;
  /** Renders a square button; pass a `title`/`aria-label` for the accessible name. */
  iconOnly?: boolean;
}

/** A pill button that sits on a glass pane: quiet rim, faint fill, stronger on hover. */
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
  const classes = [
    "gbtn",
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
