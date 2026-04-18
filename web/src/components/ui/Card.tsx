import React from "react";

interface CardProps {
  title?: string;
  subtitle?: string;
  children: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
  variant?: "default" | "elevated" | "outlined";
}

const variantStyles = {
  default: "border border-white/10 bg-black/20",
  elevated: "border border-white/20 bg-white/5 shadow-lg",
  outlined: "border-2 border-white/10 bg-transparent",
};

export const Card: React.FC<CardProps> = ({
  title,
  subtitle,
  children,
  actions,
  className = "",
  variant = "default",
}) => {
  const variantStyle = variantStyles[variant];

  return (
    <div className={`${variantStyle} ${className}`}>
      {title && (
        <div className="border-b border-white/10 px-4 py-3 sm:px-6 sm:py-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="font-archaic text-base text-white">{title}</h3>
              {subtitle && (
                <p className="mt-1 text-[10px] uppercase tracking-[0.3em] text-white/35 font-rune">
                  {subtitle}
                </p>
              )}
            </div>
            {actions && <div className="shrink-0">{actions}</div>}
          </div>
        </div>
      )}
      <div className="px-4 py-3 sm:px-6 sm:py-4">{children}</div>
    </div>
  );
};

Card.displayName = "Card";
