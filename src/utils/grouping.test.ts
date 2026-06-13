import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { groupByDate } from "./grouping";

// Tests run with TZ=UTC (vitest.config.ts) so accessed_at + "Z" and the local
// "today"/"yesterday" comparison agree. System time is pinned to noon UTC.
describe("groupByDate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-13T12:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns an empty map for no entries", () => {
    expect(groupByDate([]).size).toBe(0);
  });

  it("puts pinned entries in a Pinned group listed first", () => {
    const groups = groupByDate([
      { accessed_at: "2026-06-13 08:00:00", is_pinned: false },
      { accessed_at: "2026-01-01 08:00:00", is_pinned: true },
    ]);
    expect([...groups.keys()][0]).toBe("Pinned");
    expect(groups.get("Pinned")).toHaveLength(1);
    expect(groups.get("Pinned")![0].accessed_at).toBe("2026-01-01 08:00:00");
  });

  it("labels same-day entries Today and prior-day Yesterday", () => {
    const groups = groupByDate([
      { accessed_at: "2026-06-13 08:00:00", is_pinned: false },
      { accessed_at: "2026-06-12 23:00:00", is_pinned: false },
    ]);
    expect(groups.get("Today")).toHaveLength(1);
    expect(groups.get("Yesterday")).toHaveLength(1);
  });

  it("groups older entries under a localized date label", () => {
    const old = "2026-06-01 10:00:00";
    const groups = groupByDate([{ accessed_at: old, is_pinned: false }]);
    const expectedLabel = new Date(`${old}Z`).toLocaleDateString("zh-CN", {
      month: "long",
      day: "numeric",
      weekday: "short",
    });
    expect(groups.has(expectedLabel)).toBe(true);
    expect(groups.has("Today")).toBe(false);
    expect(groups.has("Yesterday")).toBe(false);
  });

  it("keeps multiple entries from the same day in one group", () => {
    const groups = groupByDate([
      { accessed_at: "2026-06-13 09:00:00", is_pinned: false },
      { accessed_at: "2026-06-13 10:00:00", is_pinned: false },
    ]);
    expect(groups.get("Today")).toHaveLength(2);
  });
});
