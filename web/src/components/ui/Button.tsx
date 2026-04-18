import React from "react";

type ButtonVariant = "primary" | "secondary" | "danger";
type ButtonSize = "sm" | "md" | "lg";

interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  disabled?: boolean;
  children: React.ReactNode;
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    "border-spectral/30 bg-spectral/10 text-spectral hover:border-spectral/70 disabled:opacity-50",
  secondary:
    "border-white/10 bg-white/5 text-white/80 hover:border-white/30 disabled:opacity-50",
  danger:
    "border-red-400/30 bg-red-500/10 text-red-300 hover:border-red-400/70 disabled:opacity-50",
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: "px-2 py-1 text-[10px] uppercase tracking-[0.25em]",
  md: "px-3 py-2 text-xs uppercase tracking-[0.25em]",
  lg: "px-4 py-3 text-sm uppercase tracking-[0.25em]",
};

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "primary",
      size = "md",
      loading = false,
      disabled = false,
      className = "",
      children,
      ...props
    },
    ref,
  ) => {
    const baseStyles =
      "inline-flex items-center justify-center gap-2 border font-rune transition disabled:cursor-not-allowed";
    const variantStyle = variantStyles[variant];
    const sizeStyle = sizeStyles[size];

    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={`${baseStyles} ${variantStyle} ${sizeStyle} ${className}`}
        {...props}
      >
        {loading && (
          <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent" />
        )}
        {children}
      </button>
    );
  },
);

Button.displayName = "Button";
