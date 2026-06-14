import type { StringKey } from "./strings";
import { useMemo } from "react";
import { useSettingsStore } from "../stores/settings";
import { STRINGS } from "./strings";

export type Locale = "zh" | "en";

export type { StringKey };

type Params = Record<string, string | number>;

/**
 * Pure translator. Looks up `key` in `locale`, falling back to the zh source and
 * finally the key itself, then interpolates `{name}` placeholders from `params`.
 */
export function t(locale: Locale, key: StringKey, params?: Params): string {
  const table = STRINGS[locale] ?? STRINGS.zh;
  let s: string = table[key] ?? STRINGS.zh[key] ?? key;
  if (params) {
    for (const k of Object.keys(params)) {
      s = s.split(`{${k}}`).join(String(params[k]));
    }
  }
  return s;
}

/** Current locale outside React (reads the store snapshot). */
export function getLocale(): Locale {
  return useSettingsStore.getState().settings.locale;
}

/** Reactive current locale (re-renders on change). */
export function useLocale(): Locale {
  return useSettingsStore(s => s.settings.locale);
}

/**
 * Reactive translator bound to the current locale. Stable per locale, so it's
 * safe to list in `useMemo`/`useCallback` dependency arrays.
 */
export function useT(): (key: StringKey, params?: Params) => string {
  const locale = useLocale();
  return useMemo(() => (key: StringKey, params?: Params) => t(locale, key, params), [locale]);
}
