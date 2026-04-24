import type { FormHTMLAttributes, InputHTMLAttributes, LabelHTMLAttributes } from "react";

export function Form({ className = "", ...props }: FormHTMLAttributes<HTMLFormElement>) {
  return <form className={`space-y-4 ${className}`} {...props} />;
}

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export function Input({ label, error, id, className = "", ...props }: InputProps) {
  return (
    <div className="space-y-1">
      {label && (
        <label htmlFor={id} className="block text-sm font-medium text-neriak-muted">
          {label}
        </label>
      )}
      <input
        id={id}
        className={`w-full px-3 py-2 bg-void border border-neriak-dim rounded text-neriak-text
          placeholder-neriak-muted focus:outline-none focus:ring-2 focus:ring-neriak-cyan focus:border-transparent
          ${error ? "border-state-danger" : ""} ${className}`}
        {...props}
      />
      {error && <p className="text-sm text-state-danger">{error}</p>}
    </div>
  );
}

export function Label({ className = "", ...props }: LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label className={`block text-sm font-medium text-neriak-muted ${className}`} {...props} />
  );
}
