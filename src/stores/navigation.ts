import { create } from "zustand";

export type ViewName = "clipboard" | "settings" | "about";

interface NavigationStore {
  currentView: ViewName;
  navigateTo: (view: ViewName) => void;
}

export const useNavigationStore = create<NavigationStore>(set => ({
  currentView: "clipboard",
  navigateTo: view => set({ currentView: view }),
}));
