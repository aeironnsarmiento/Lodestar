import { forwardRef, useEffect, useState, type InputHTMLAttributes, type SelectHTMLAttributes } from "react";

export const GlassInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function GlassInput({ className, ...rest }, ref) {
    return <input ref={ref} className={["ginput", className].filter(Boolean).join(" ")} {...rest} />;
  },
);

export function GlassSelect({ className, children, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={["ginput", className].filter(Boolean).join(" ")} {...rest}>
      {children}
    </select>
  );
}

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}

export function Switch({ checked, onChange, label, disabled }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className="switch"
      onClick={() => onChange(!checked)}
    />
  );
}

interface NumberInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "type"> {
  value: number;
  onValue: (value: number) => void;
  min: number;
  max: number;
}

/**
 * A number field that lets the user clear and retype freely. Only whole numbers in
 * range are committed; leaving the field restores the last committed value.
 */
export function NumberInput({ value, onValue, min, max, ...rest }: NumberInputProps) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <GlassInput
      {...rest}
      type="number"
      min={min}
      max={max}
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        const n = Number(e.target.value);
        if (e.target.value !== "" && Number.isInteger(n) && n >= min && n <= max) onValue(n);
      }}
      onBlur={() => setText(String(value))}
    />
  );
}
