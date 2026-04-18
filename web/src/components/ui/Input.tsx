import React from "react";

type InputType = "text" | "password" | "email" | "number";

interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  type?: InputType;
  label?: string;
  error?: string;
  hint?: string;
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ type = "text", label, error, hint, className = "", ...props }, ref) => {
    const baseStyles =
      "w-full border border-white/10 bg-black/20 px-3 py-2 text-sm text-white outline-none transition focus:border-magentaglow/60 focus:bg-black/30";

    const inputClass = error
      ? "border-red-400/40 focus:border-red-400/60"
      : baseStyles;

    return (
      <div className="flex flex-col gap-2">
        {label && (
          <label className="text-[11px] uppercase tracking-[0.25em] text-white/60 font-rune">
            {label}
            {hint && (
              <span className="ml-2 text-[10px] text-white/30">{hint}</span>
            )}
          </label>
        )}
        <input
          ref={ref}
          type={type}
          className={`${inputClass} ${className}`}
          {...props}
        />
        {error && (
          <span className="text-[11px] text-red-300">{error}</span>
        )}
      </div>
    );
  },
);

Input.displayName = "Input";
