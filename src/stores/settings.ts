import type { Locale } from "../i18n";
import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { create } from "zustand";

export type ThemeMode = "system" | "dark" | "light";

/** Predefined accent color presets (oklch hue values). Display names are
 *  resolved per-locale via the i18n `color.*` keys (see AccentColorPicker). */
export const ACCENT_PRESETS = [
  { id: "blue", hue: 260, swatch: "oklch(0.65 0.18 260)" },
  { id: "purple", hue: 290, swatch: "oklch(0.65 0.18 290)" },
  { id: "pink", hue: 340, swatch: "oklch(0.65 0.18 340)" },
  { id: "red", hue: 25, swatch: "oklch(0.65 0.18 25)" },
  { id: "orange", hue: 55, swatch: "oklch(0.72 0.18 55)" },
  { id: "green", hue: 150, swatch: "oklch(0.65 0.18 150)" },
  { id: "teal", hue: 195, swatch: "oklch(0.65 0.15 195)" },
] as const;

export type AccentColorId = typeof ACCENT_PRESETS[number]["id"];

export interface AppSettings {
  history_retention_days: number;
  max_history_count: number;
  default_action: "copy" | "paste";
  theme: ThemeMode;
  global_shortcut: string;
  accent_color: AccentColorId;
  show_menu_bar_icon: boolean;
  locale: Locale;
}

interface SettingsStore {
  settings: AppSettings;
  isLoading: boolean;
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>;
  loadSettings: () => Promise<void>;
}

const DEFAULT_SETTINGS: AppSettings = {
  // Unlimited by default (-1): keep all history until the user opts into a cap.
  history_retention_days: -1,
  max_history_count: -1,
  default_action: "paste",
  theme: "system",
  global_shortcut: "CmdOrCtrl+Shift+V",
  accent_color: "blue",
  show_menu_bar_icon: true,
  locale: "zh",
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
      // Start from the frontend defaults so frontend-only keys (e.g. locale,
      // which the Rust AppSettings doesn't carry) get a value, then let the
      // backend defaults and finally the persisted store override.
      const loadedSettings = { ...DEFAULT_SETTINGS, ...backendDefaults };

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
    const previous = useSettingsStore.getState().settings[key];

    // Optimistic UI update.
    set(state => ({
      settings: {
        ...state.settings,
        [key]: value,
      },
    }));

    try {
      // Run side effects that can FAIL (and that validate the value) BEFORE
      // persisting, so a rejected value is never written to disk. A bad
      // global_shortcut persisted here would otherwise break the next launch.
      if (key === "global_shortcut") {
        await invoke("update_global_shortcut", { shortcut: value });
      }

      if (key === "show_menu_bar_icon") {
        await invoke("set_tray_visible", { visible: value });
      }

      if (!storeInstance) {
        storeInstance = await Store.load("settings.json", { defaults: {}, autoSave: true });
      }

      await storeInstance.set(key, value);
      await storeInstance.save();

      // Appearance side effects can't fail — apply after persisting.
      if (key === "theme" || key === "accent_color") {
        const currentSettings = useSettingsStore.getState().settings;
        applyAppearance(currentSettings.theme, currentSettings.accent_color);
      }

      // Rebuild the native tray + app menu so they switch language too. Runs
      // after persisting so the Rust side reads the new locale from settings.json.
      if (key === "locale") {
        await invoke("relocalize_menus");
      }
    } catch (err) {
      // Revert the optimistic update so a rejected value isn't shown or saved.
      set(state => ({
        settings: {
          ...state.settings,
          [key]: previous,
        },
      }));
      console.error(`Failed to update setting ${key}:`, err);
      throw err;
    }
  },
}));
