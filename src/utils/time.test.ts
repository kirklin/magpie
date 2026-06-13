import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { formatByteSize, formatRelativeTime } from "./time";

// Stored timestamps are UTC strings like "2026-06-13 12:00:00" (no zone);
// formatRelativeTime appends "Z" to treat them as UTC.
describe("formatRelativeTime", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-13T12:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns 刚刚 for under 10 seconds", () => {
    expect(formatRelativeTime("2026-06-13 11:59:55")).toBe("刚刚");
  });

  it("returns seconds for under a minute", () => {
    expect(formatRelativeTime("2026-06-13 11:59:30")).toBe("30秒前");
  });

  it("returns minutes for under an hour", () => {
    expect(formatRelativeTime("2026-06-13 11:30:00")).toBe("30分钟前");
  });

  it("returns hours for under a day", () => {
    expect(formatRelativeTime("2026-06-13 09:00:00")).toBe("3小时前");
  });

  it("returns days for under a week", () => {
    expect(formatRelativeTime("2026-06-10 12:00:00")).toBe("3天前");
  });

  it("returns weeks for under 30 days", () => {
    expect(formatRelativeTime("2026-06-03 12:00:00")).toBe("1周前");
  });

  it("falls back to a localized date past 30 days", () => {
    const old = "2026-04-01 12:00:00";
    const expected = new Date(`${old}Z`).toLocaleDateString("zh-CN");
    expect(formatRelativeTime(old)).toBe(expected);
  });
});

describe("formatByteSize", () => {
  it("formats bytes", () => {
    expect(formatByteSize(0)).toBe("0 B");
    expect(formatByteSize(512)).toBe("512 B");
    expect(formatByteSize(1023)).toBe("1023 B");
  });

  it("formats kilobytes with one decimal", () => {
    expect(formatByteSize(1024)).toBe("1.0 KB");
    expect(formatByteSize(1536)).toBe("1.5 KB");
  });

  it("formats megabytes with one decimal", () => {
    expect(formatByteSize(1024 * 1024)).toBe("1.0 MB");
    expect(formatByteSize(1.5 * 1024 * 1024)).toBe("1.5 MB");
  });
});
