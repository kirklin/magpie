import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { create } from "zustand";

export type ThemeMode = "system" | "dark" | "light";

export interface AppSettings {
  history_retention_days: number;
  max_history_count: number;
  default_action: "copy" | "paste";
  theme: ThemeMode;
  global_shortcut: string;
}

interface SettingsStore {
  settings: AppSettings;
  isLoading: boolean;
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>;
  loadSettings: () => Promise<void>;
}

const DEFAULT_SETTINGS: AppSettings = {
  history_retention_days: 30,
  max_history_count: 1000,
  default_action: "paste",
  theme: "system",
  global_shortcut: "CmdOrCtrl+Shift+V",
};

let storeInstance: Store | null = null;

/**
 * Resolve the effective theme ("dark" | "light") from the user's preference.
 * "system" defers to the OS `prefers-color-scheme` media query.
 */
function resolveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

/** Apply the theme to the DOM by setting data-theme on <html> */
function applyTheme(mode: ThemeMode) {
  const resolved = resolveTheme(mode);
  document.documentElement.setAttribute("data-theme", resolved);
}

// Listen for system theme changes to auto-update when mode is "system"
if (typeof window !== "undefined") {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    const state = useSettingsStore.getState();
    if (state.settings.theme === "system") {
      applyTheme("system");
    }
  });
}

export const useSettingsStore = create<SettingsStore>(set => ({
  settings: DEFAULT_SETTINGS,
  isLoading: true,

  loadSettings: async () => {
    try {
      if (!storeInstance) {
        storeInstance = await Store.load("settings.json", { defaults: {}, autoSave: true });
      }

      // Get defaults from backend
      const backendDefaults = await invoke<AppSettings>("get_default_settings").catch(() => DEFAULT_SETTINGS);

      const loadedSettings = { ...backendDefaults };

      // Override with local saved values
      for (const key of Object.keys(DEFAULT_SETTINGS) as Array<keyof AppSettings>) {
        const val = await storeInstance.get(key);
        if (val !== undefined && val !== null) {
          (loadedSettings as any)[key] = val;
        }
      }

      // Apply theme from loaded settings
      applyTheme(loadedSettings.theme as ThemeMode ?? "system");

      set({ settings: loadedSettings as AppSettings, isLoading: false });
    } catch (err) {
      console.error("Failed to load settings:", err);
      set({ isLoading: false });
    }
  },

  updateSetting: async (key, value) => {
    try {
      set(state => ({
        settings: {
          ...state.settings,
          [key]: value,
        },
      }));

      if (!storeInstance) {
        storeInstance = await Store.load("settings.json", { defaults: {}, autoSave: true });
      }

      await storeInstance.set(key, value);
      await storeInstance.save();

      // Side effects
      if (key === "theme") {
        applyTheme(value as ThemeMode);
      }

      if (key === "global_shortcut") {
        // Re-register the global shortcut via Rust backend
        await invoke("update_global_shortcut", { shortcut: value });
      }
    } catch (err) {
      console.error(`Failed to update setting ${key}:`, err);
      throw err; // Re-throw so callers can handle
    }
  },
}));
