import type { Locale } from "../i18n";
import { t } from "../i18n";

/** Format relative time in the given locale (defaults to zh). */
export function formatRelativeTime(dateStr: string, locale: Locale = "zh"): string {
  const date = new Date(`${dateStr}Z`); // UTC
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 10) {
    return t(locale, "time.just_now");
  }
  if (diffSec < 60) {
    return t(locale, "time.sec_ago", { n: diffSec });
  }
  if (diffMin < 60) {
    return t(locale, "time.min_ago", { n: diffMin });
  }
  if (diffHour < 24) {
    return t(locale, "time.hour_ago", { n: diffHour });
  }
  if (diffDay < 7) {
    return t(locale, "time.day_ago", { n: diffDay });
  }
  if (diffDay < 30) {
    return t(locale, "time.week_ago", { n: Math.floor(diffDay / 7) });
  }
  return date.toLocaleDateString(locale === "zh" ? "zh-CN" : "en-US");
}

/** Format byte size */
export function formatByteSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
