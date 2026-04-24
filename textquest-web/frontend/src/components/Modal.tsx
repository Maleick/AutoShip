import type { ReactNode } from "react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: ReactNode;
}

export function Modal({ open, onClose, title, children }: ModalProps) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? "modal-title" : undefined}
    >
      <div className="absolute inset-0 bg-void/80" onClick={onClose} />
      <div className="relative bg-panel border border-neriak-magenta/40 rounded-lg p-6 max-w-lg w-full mx-4 shadow-xl">
        <button
          type="button"
          onClick={onClose}
          aria-label="Close"
          className="absolute top-3 right-3 text-neriak-muted hover:text-neriak-text focus:outline-none focus:ring-2 focus:ring-neriak-cyan rounded"
        >
          ×
        </button>
        {title && (
          <h2 id="modal-title" className="text-lg font-semibold text-neriak-text mb-4 pr-8">
            {title}
          </h2>
        )}
        {children}
      </div>
    </div>
  );
}
