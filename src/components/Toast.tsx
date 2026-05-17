import { useEffect, useState } from "react";
import { create } from "zustand";

// --- Toast Store ---

interface Toast {
  id: number;
  message: string;
  type: "success" | "info" | "error";
}

interface ToastStore {
  toasts: Toast[];
  add: (message: string, type?: Toast["type"]) => void;
  remove: (id: number) => void;
}

let nextId = 0;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  add: (message, type = "success") => {
    const id = nextId++;
    set(state => ({ toasts: [...state.toasts, { id, message, type }] }));
    // Auto-remove after 2s
    setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
    }, 2000);
  },
  remove: (id) => set(state => ({ toasts: state.toasts.filter(t => t.id !== id) })),
}));

// --- Toast Container Component ---

export function ToastContainer() {
  const { toasts } = useToastStore();

  if (toasts.length === 0) return null;

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
    const timer = setTimeout(() => setVisible(false), 1700);
    return () => clearTimeout(timer);
  }, []);

  const bgColor = toast.type === "error"
    ? "bg-red-500/90"
    : toast.type === "info"
      ? "bg-white/10"
      : "bg-emerald-500/90";

  return (
    <div
      className={`px-3.5 py-1.5 rounded-lg text-[12px] font-medium text-white shadow-lg backdrop-blur-xl border border-white/10 transition-all duration-200 ${bgColor} ${
        visible
          ? "opacity-100 translate-y-0"
          : "opacity-0 -translate-y-2"
      }`}
    >
      {toast.message}
    </div>
  );
}
