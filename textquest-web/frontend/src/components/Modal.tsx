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
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />
      <div className="relative bg-gray-900 border border-gray-700 rounded-lg p-6 max-w-lg w-full mx-4 shadow-xl">
        {title && (
          <h2 className="text-lg font-semibold text-gray-100 mb-4">{title}</h2>
        )}
        {children}
      </div>
    </div>
  );
}
