import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { create } from "zustand";

export type ThemeMode = "system" | "dark" | "light";

/** Predefined accent color presets (oklch hue values) */
export const ACCENT_PRESETS = [
  { id: "blue", hue: 260, label: "蓝色", swatch: "oklch(0.65 0.18 260)" },
  { id: "purple", hue: 290, label: "紫色", swatch: "oklch(0.65 0.18 290)" },
  { id: "pink", hue: 340, label: "粉色", swatch: "oklch(0.65 0.18 340)" },
  { id: "red", hue: 25, label: "红色", swatch: "oklch(0.65 0.18 25)" },
  { id: "orange", hue: 55, label: "橙色", swatch: "oklch(0.72 0.18 55)" },
  { id: "green", hue: 150, label: "绿色", swatch: "oklch(0.65 0.18 150)" },
  { id: "teal", hue: 195, label: "青色", swatch: "oklch(0.65 0.15 195)" },
] as const;

export type AccentColorId = typeof ACCENT_PRESETS[number]["id"];

export interface AppSettings {
  history_retention_days: number;
  max_history_count: number;
  default_action: "copy" | "paste";
  theme: ThemeMode;
  global_shortcut: string;
  accent_color: AccentColorId;
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
  accent_color: "blue",
};

let storeInstance: Store | null = null;

/**
 * Resolve the effective theme ("dark" | "light") from the user's preference.
 */
function resolveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

/**
 * Apply the accent color by setting CSS custom properties on <html>.
 * All accent-related tokens are derived from a single oklch hue value.
 */
function applyAccentColor(accentId: AccentColorId, themeMode: ThemeMode) {
  const preset = ACCENT_PRESETS.find(p => p.id === accentId) ?? ACCENT_PRESETS[0];
  const h = preset.hue;
  const resolved = resolveTheme(themeMode);
  const el = document.documentElement;

  if (resolved === "dark") {
    el.style.setProperty("--color-accent", `oklch(0.65 0.18 ${h})`);
    el.style.setProperty("--color-accent-hover", `oklch(0.72 0.16 ${h})`);
    el.style.setProperty("--color-accent-muted", `oklch(0.65 0.18 ${h} / 0.15)`);
    el.style.setProperty("--color-border-focused", `oklch(0.65 0.18 ${h} / 0.5)`);
    el.style.setProperty("--color-text-accent", `oklch(0.75 0.12 ${h})`);
    el.style.setProperty("--color-bg-selected", `oklch(0.6 0.15 ${h} / 0.2)`);
  } else {
    el.style.setProperty("--color-accent", `oklch(0.55 0.2 ${h})`);
    el.style.setProperty("--color-accent-hover", `oklch(0.5 0.18 ${h})`);
    el.style.setProperty("--color-accent-muted", `oklch(0.55 0.2 ${h} / 0.12)`);
    el.style.setProperty("--color-border-focused", `oklch(0.55 0.2 ${h} / 0.5)`);
    el.style.setProperty("--color-text-accent", `oklch(0.45 0.15 ${h})`);
    el.style.setProperty("--color-bg-selected", `oklch(0.6 0.15 ${h} / 0.12)`);
  }
}

/** Apply theme + accent together */
function applyAppearance(themeMode: ThemeMode, accentId: AccentColorId) {
  const resolved = resolveTheme(themeMode);
  document.documentElement.setAttribute("data-theme", resolved);
  applyAccentColor(accentId, themeMode);
}

// Listen for system theme changes
if (typeof window !== "undefined") {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    const state = useSettingsStore.getState();
    if (state.settings.theme === "system") {
      applyAppearance("system", state.settings.accent_color);
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

      const backendDefaults = await invoke<AppSettings>("get_default_settings").catch(() => DEFAULT_SETTINGS);
      const loadedSettings = { ...backendDefaults };

      for (const key of Object.keys(DEFAULT_SETTINGS) as Array<keyof AppSettings>) {
        const val = await storeInstance.get(key);
        if (val !== undefined && val !== null) {
          (loadedSettings as any)[key] = val;
        }
      }

      // Apply appearance from loaded settings
      const theme = (loadedSettings.theme as ThemeMode) ?? "system";
      const accent = (loadedSettings.accent_color as AccentColorId) ?? "blue";
      applyAppearance(theme, accent);

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

      // Side effects — re-apply appearance for theme or accent changes
      const currentSettings = useSettingsStore.getState().settings;

      if (key === "theme" || key === "accent_color") {
        applyAppearance(currentSettings.theme, currentSettings.accent_color);
      }

      if (key === "global_shortcut") {
        await invoke("update_global_shortcut", { shortcut: value });
      }
    } catch (err) {
      console.error(`Failed to update setting ${key}:`, err);
      throw err;
    }
  },
}));
