/** Group entries by date, with pinned items in a dedicated top group */
export function groupByDate(entries: Array<{ accessed_at: string; is_pinned?: boolean }>): Map<string, typeof entries> {
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
    groups.set("Pinned", pinned);
  }

  for (const entry of unpinned) {
    const date = new Date(`${entry.accessed_at}Z`);
    const dateStr = date.toDateString();

    let label: string;
    if (dateStr === today) {
      label = "Today";
    } else if (dateStr === yesterdayStr) {
      label = "Yesterday";
    } else {
      label = date.toLocaleDateString("zh-CN", {
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
