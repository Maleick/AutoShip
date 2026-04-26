import type { FormHTMLAttributes, InputHTMLAttributes, LabelHTMLAttributes } from "react";
import { ReactNode } from "react";

interface FormProps extends FormHTMLAttributes<HTMLFormElement> {
  isSubmitting?: boolean;
}

export function Form({ className = "", isSubmitting = false, ...props }: FormProps) {
  return (
    <form
      className={`space-y-4 ${className}`}
      {...props}
      aria-busy={isSubmitting}
    />
  );
}

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  required?: boolean;
  helperText?: string;
}

export function Input({
  label,
  error,
  id,
  className = "",
  required = false,
  helperText,
  ...props
}: InputProps) {
  return (
    <div className="space-y-1">
      {label && (
        <label htmlFor={id} className="block text-sm font-medium text-neriak-muted">
          {label}
          {required && <span className="text-state-danger"> *</span>}
        </label>
      )}
      <input
        id={id}
        className={`w-full px-3 py-2 bg-void border border-neriak-dim rounded text-neriak-text
          placeholder-neriak-muted transition-colors
          ${error ? "border-state-danger bg-state-danger/10" : "focus:border-neriak-bright"} ${className}`}
        aria-invalid={!!error}
        aria-describedby={error ? `${id}-error` : helperText ? `${id}-helper` : undefined}
        {...props}
      />
      {error && (
        <p
          id={`${id}-error`}
          role="alert"
          aria-live="assertive"
          className="text-sm text-state-danger"
        >
          {error}
        </p>
      )}
      {helperText && !error && (
        <p id={`${id}-helper`} className="text-sm text-neriak-muted">
          {helperText}
        </p>
      )}
    </div>
  );
}

interface SelectProps extends InputHTMLAttributes<HTMLSelectElement> {
  label?: string;
  error?: string;
  required?: boolean;
  options: Array<{ value: string; label: string }>;
  placeholder?: string;
}

export function Select({
  label,
  error,
  id,
  className = "",
  required = false,
  options,
  placeholder,
  ...props
}: SelectProps) {
  return (
    <div className="space-y-1">
      {label && (
        <label htmlFor={id} className="block text-sm font-medium text-neriak-muted">
          {label}
          {required && <span className="text-state-danger"> *</span>}
        </label>
      )}
      <select
        id={id}
        className={`w-full px-3 py-2 bg-void border border-neriak-dim rounded text-neriak-text
          transition-colors
          ${error ? "border-state-danger bg-state-danger/10" : "focus:border-neriak-bright"} ${className}`}
        aria-invalid={!!error}
        aria-describedby={error ? `${id}-error` : undefined}
        {...props}
      >
        {placeholder && <option value="">{placeholder}</option>}
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      {error && (
        <p
          id={`${id}-error`}
          role="alert"
          aria-live="assertive"
          className="text-sm text-state-danger"
        >
          {error}
        </p>
      )}
    </div>
  );
}

interface TextAreaProps extends InputHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  required?: boolean;
}

export function TextArea({
  label,
  error,
  id,
  className = "",
  required = false,
  ...props
}: TextAreaProps) {
  return (
    <div className="space-y-1">
      {label && (
        <label htmlFor={id} className="block text-sm font-medium text-neriak-muted">
          {label}
          {required && <span className="text-state-danger"> *</span>}
        </label>
      )}
      <textarea
        id={id}
        className={`w-full px-3 py-2 bg-void border border-neriak-dim rounded text-neriak-text
          placeholder-neriak-muted transition-colors resize-vertical
          ${error ? "border-state-danger bg-state-danger/10" : "focus:border-neriak-bright"} ${className}`}
        aria-invalid={!!error}
        aria-describedby={error ? `${id}-error` : undefined}
        {...(props as any)}
      />
      {error && (
        <p
          id={`${id}-error`}
          role="alert"
          aria-live="assertive"
          className="text-sm text-state-danger"
        >
          {error}
        </p>
      )}
    </div>
  );
}

interface FormGroupProps {
  children: ReactNode;
  className?: string;
}

export function FormGroup({ children, className = "" }: FormGroupProps) {
  return <div className={`space-y-4 ${className}`}>{children}</div>;
}

export function Label({ className = "", ...props }: LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label className={`block text-sm font-medium text-neriak-muted ${className}`} {...props} />
  );
}
