import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardEntry } from "./clipboard";
import { useClipboardStore } from "./clipboard";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function makeEntry(over: Partial<ClipboardEntry> & { id: number; accessed_at: string }): ClipboardEntry {
  return {
    content_type: "text",
    text_content: "x",
    html_content: null,
    image_path: null,
    file_paths: null,
    source_app: null,
    source_app_name: null,
    custom_name: null,
    is_pinned: false,
    is_favorite: false,
    content_hash: `h${over.id}`,
    content_preview: null,
    byte_size: 0,
    created_at: over.accessed_at,
    access_count: 1,
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  useClipboardStore.setState({ entries: [], selectedId: null, isLoadingMore: false });
});

describe("addNewEntry", () => {
  it("prepends a new entry, newest accessed_at first", () => {
    useClipboardStore.setState({ entries: [makeEntry({ id: 1, accessed_at: "2026-06-13 10:00:00" })] });
    useClipboardStore.getState().addNewEntry(makeEntry({ id: 2, accessed_at: "2026-06-13 11:00:00" }));
    expect(useClipboardStore.getState().entries.map(e => e.id)).toEqual([2, 1]);
  });

  it("merges by id rather than duplicating, keeping the newer payload", () => {
    useClipboardStore.setState({ entries: [makeEntry({ id: 1, accessed_at: "2026-06-13 10:00:00", text_content: "old" })] });
    useClipboardStore.getState().addNewEntry(makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00", text_content: "new" }));
    const entries = useClipboardStore.getState().entries;
    expect(entries).toHaveLength(1);
    expect(entries[0].text_content).toBe("new");
  });

  it("sorts pinned entries above unpinned regardless of recency", () => {
    useClipboardStore.getState().addNewEntry(makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00", is_pinned: false }));
    useClipboardStore.getState().addNewEntry(makeEntry({ id: 2, accessed_at: "2026-01-01 00:00:00", is_pinned: true }));
    expect(useClipboardStore.getState().entries.map(e => e.id)).toEqual([2, 1]);
  });
});

describe("togglePin", () => {
  it("applies the backend's pin state and re-sorts pinned first", async () => {
    vi.mocked(invoke).mockResolvedValue(true);
    useClipboardStore.setState({ entries: [
      makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00" }),
      makeEntry({ id: 2, accessed_at: "2026-06-13 08:00:00" }),
    ] });
    await useClipboardStore.getState().togglePin(2);
    const entries = useClipboardStore.getState().entries;
    expect(entries[0].id).toBe(2);
    expect(entries[0].is_pinned).toBe(true);
    expect(invoke).toHaveBeenCalledWith("toggle_pin_entry", { id: 2 });
  });
});

describe("deleteEntry", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("removes the entry and selects the item that took its place", async () => {
    useClipboardStore.setState({
      entries: [
        makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00" }),
        makeEntry({ id: 2, accessed_at: "2026-06-13 11:00:00" }),
        makeEntry({ id: 3, accessed_at: "2026-06-13 10:00:00" }),
      ],
      selectedId: 2,
    });
    await useClipboardStore.getState().deleteEntry(2);
    const s = useClipboardStore.getState();
    expect(s.entries.map(e => e.id)).toEqual([1, 3]);
    expect(s.selectedId).toBe(3);
  });

  it("selects the new last item when deleting the last entry", async () => {
    useClipboardStore.setState({
      entries: [
        makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00" }),
        makeEntry({ id: 2, accessed_at: "2026-06-13 11:00:00" }),
      ],
      selectedId: 2,
    });
    await useClipboardStore.getState().deleteEntry(2);
    expect(useClipboardStore.getState().selectedId).toBe(1);
  });

  it("leaves selection unchanged when deleting a non-selected entry", async () => {
    useClipboardStore.setState({
      entries: [
        makeEntry({ id: 1, accessed_at: "2026-06-13 12:00:00" }),
        makeEntry({ id: 2, accessed_at: "2026-06-13 11:00:00" }),
      ],
      selectedId: 1,
    });
    await useClipboardStore.getState().deleteEntry(2);
    expect(useClipboardStore.getState().selectedId).toBe(1);
  });
});
