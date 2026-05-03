/** Group entries by date */
export function groupByDate(entries: Array<{ created_at: string }>): Map<string, typeof entries> {
  const groups = new Map<string, typeof entries>();
  const now = new Date();
  const today = now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const yesterdayStr = yesterday.toDateString();

  for (const entry of entries) {
    const date = new Date(`${entry.created_at}Z`);
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
