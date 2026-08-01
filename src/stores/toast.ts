import { create } from "zustand";

export interface Toast {
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

// Lives here rather than beside ToastContainer so that file exports only
// components — otherwise Fast Refresh drops the whole module on every edit.
export const useToastStore = create<ToastStore>(set => ({
  toasts: [],
  add: (message, type = "success") => {
    const id = nextId++;
    set(state => ({ toasts: [...state.toasts, { id, message, type }] }));
    // Auto-remove after 2s
    setTimeout(() => {
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
    }, 2000);
  },
  remove: id => set(state => ({ toasts: state.toasts.filter(t => t.id !== id) })),
}));
