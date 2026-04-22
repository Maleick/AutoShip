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
        <label htmlFor={id} className="block text-sm font-medium text-gray-300">
          {label}
        </label>
      )}
      <input
        id={id}
        className={`w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-gray-100
          placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent
          ${error ? "border-red-500" : ""} ${className}`}
        {...props}
      />
      {error && <p className="text-sm text-red-400">{error}</p>}
    </div>
  );
}

export function Label({ className = "", ...props }: LabelHTMLAttributes<HTMLLabelElement>) {
  return <label className={`block text-sm font-medium text-gray-300 ${className}`} {...props} />;
}
