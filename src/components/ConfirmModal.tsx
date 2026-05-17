import { AlertTriangle } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";

interface ConfirmModalProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * In-app confirmation dialog. Required because window.confirm() gets
 * hidden behind Tauri's always-on-top window.
 */
export function ConfirmModal({
  isOpen,
  title,
  message,
  confirmLabel = "Confirm",
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (isOpen) {
      requestAnimationFrame(() => confirmRef.current?.focus());
    }
  }, [isOpen]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!isOpen) return;
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        onConfirm();
      }
    },
    [isOpen, onConfirm, onCancel],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  if (!isOpen) return null;

  return (
    <>
      <div
        className="fixed inset-0 z-[60] bg-black/50 backdrop-blur-sm"
        onClick={onCancel}
      />
      <div className="fixed z-[70] top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[340px] bg-[#1E1E1E]/98 backdrop-blur-xl border border-white/10 rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.5)] animate-scale-in p-5">
        <div className="flex items-start gap-3 mb-4">
          <div className="shrink-0 w-8 h-8 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <h3 className="text-[14px] font-semibold text-white/90 mb-1">{title}</h3>
            <p className="text-[13px] text-white/50 leading-relaxed">{message}</p>
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <button
            className="px-3 py-1.5 text-[13px] text-white/60 hover:text-white/90 rounded-lg hover:bg-white/5 transition-colors"
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            ref={confirmRef}
            className="px-3 py-1.5 text-[13px] text-white font-medium bg-red-500/80 hover:bg-red-500 rounded-lg transition-colors"
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </>
  );
}
