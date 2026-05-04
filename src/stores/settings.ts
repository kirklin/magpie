import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { create } from "zustand";

export interface AppSettings {
  history_retention_days: number;
  max_history_count: number;
  default_action: "copy" | "paste";
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
};

let storeInstance: Store | null = null;

export const useSettingsStore = create<SettingsStore>((set) => ({
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
      for (const key of Object.keys(backendDefaults) as Array<keyof AppSettings>) {
        const val = await storeInstance.get(key);
        if (val !== undefined && val !== null) {
          (loadedSettings as any)[key] = val;
        }
      }

      set({ settings: loadedSettings, isLoading: false });
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
    } catch (err) {
      console.error(`Failed to update setting ${key}:`, err);
    }
  },
}));
