import type { Toast } from "../stores/toast";
import { useEffect, useState } from "react";
import { useToastStore } from "../stores/toast";

// --- Toast Container Component ---

export function ToastContainer() {
  const { toasts } = useToastStore();

  if (toasts.length === 0) {
    return null;
  }

  return (
    <div className="fixed top-3 left-1/2 -translate-x-1/2 z-[80] flex flex-col items-center gap-1.5 pointer-events-none">
      {toasts.map(toast => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  );
}

function ToastItem({ toast }: { toast: Toast }) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    // Trigger enter animation
    requestAnimationFrame(() => setVisible(true));
    // Trigger exit animation before removal
    const timer = setTimeout(setVisible, 1700, false);
    return () => clearTimeout(timer);
  }, []);

  const bgColor = toast.type === "error"
    ? "bg-red-500/90"
    : toast.type === "info"
      ? "bg-bg-secondary/90"
      : "bg-emerald-500/90";

  return (
    <div
      className={`px-3.5 py-1.5 rounded-lg text-[12px] font-medium shadow-lg backdrop-blur-xl border border-border transition-all duration-200 ${bgColor} ${
        toast.type === "info" ? "text-text-primary" : "text-white"
      } ${
        visible
          ? "opacity-100 translate-y-0"
          : "opacity-0 -translate-y-2"
      }`}
    >
      {toast.message}
    </div>
  );
}
