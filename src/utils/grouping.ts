import type { Locale } from "../i18n";
import { t } from "../i18n";

/** Group entries by date, with pinned items in a dedicated top group */
export function groupByDate(
  entries: Array<{ accessed_at: string; is_pinned?: boolean }>,
  locale: Locale = "zh",
): Map<string, typeof entries> {
  const groups = new Map<string, typeof entries>();
  const now = new Date();
  const today = now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const yesterdayStr = yesterday.toDateString();

  // Separate pinned entries into their own top group
  const pinned = entries.filter(e => e.is_pinned);
  const unpinned = entries.filter(e => !e.is_pinned);

  if (pinned.length > 0) {
    groups.set(t(locale, "group.pinned"), pinned);
  }

  for (const entry of unpinned) {
    const date = new Date(`${entry.accessed_at}Z`);
    const dateStr = date.toDateString();

    let label: string;
    if (dateStr === today) {
      label = t(locale, "group.today");
    } else if (dateStr === yesterdayStr) {
      label = t(locale, "group.yesterday");
    } else {
      label = date.toLocaleDateString(locale === "zh" ? "zh-CN" : "en-US", {
        month: "long",
        day: "numeric",
        weekday: "short",
      });
    }

    if (!groups.has(label)) {
      groups.set(label, []);
    }
    groups.get(label)!.push(entry);
  }

  return groups;
}
