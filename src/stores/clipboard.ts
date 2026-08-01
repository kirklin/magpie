// IPC types are generated from the Rust structs — see src/bindings.ts,
// regenerated via `cargo test export_typescript_bindings`. Imported for local
// use and re-exported so existing
// `import { ClipboardEntry } from "../stores/clipboard"` keeps working.
import type { ClipboardEntry, ClipboardQuery } from "../bindings";
import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { parseAppError } from "../lib/error";

import { useToastStore } from "./toast";

export type { ClipboardEntry, ClipboardQuery };

/** Number of entries fetched per page. The list pages in more on scroll. */
export const PAGE_SIZE = 100;

// Monotonic id for the latest first-page fetch. Tauri's invoke can't be aborted,
// so instead of cancelling an in-flight query we tag each one and drop any
// response whose id is no longer current (out-of-order / superseded results).
let fetchSeq = 0;

interface ClipboardStore {
  entries: ClipboardEntry[];
  selectedId: number | null;
  searchQuery: string;
  activeFilter: string | null;
  isLoading: boolean;
  isLoadingMore: boolean;
  hasMore: boolean;
  activeApp: string;

  setActiveApp: (app: string) => void;
  setSearchQuery: (query: string) => void;
  setActiveFilter: (filter: string | null) => void;
  setSelectedId: (id: number | null) => void;
  fetchEntries: () => Promise<void>;
  loadMore: () => Promise<void>;
  deleteEntry: (id: number) => Promise<void>;
  togglePin: (id: number) => Promise<void>;
  clearHistory: () => Promise<void>;
  pasteEntry: (text: string) => Promise<void>;
  copyEntry: (text: string) => Promise<void>;
  pasteAsPlainText: (text: string) => Promise<void>;
  pasteFileEntry: (filePathsJson: string) => Promise<void>;
  copyFileEntry: (filePathsJson: string) => Promise<void>;
  pasteImageEntry: (imagePath: string) => Promise<void>;
  copyImageEntry: (imagePath: string) => Promise<void>;
  updateEntryContent: (id: number, content: string) => Promise<void>;
  appendToClipboard: (text: string) => Promise<void>;
  saveAsFile: (content: string, defaultName: string) => Promise<boolean>;
  pasteAndKeepWindow: (text: string) => Promise<void>;
  pasteImageAndKeepWindow: (imagePath: string) => Promise<void>;
  pasteFileAndKeepWindow: (filePathsJson: string) => Promise<void>;
  addNewEntry: (entry: ClipboardEntry) => void;
  exportHistory: () => Promise<number>;
  importHistory: () => Promise<number>;
}

export const useClipboardStore = create<ClipboardStore>((set, get) => ({
  entries: [],
  selectedId: null,
  searchQuery: "",
  activeFilter: null,
  isLoading: false,
  isLoadingMore: false,
  hasMore: true,
  activeApp: "Active App",

  setActiveApp: app => set({ activeApp: app }),

  setSearchQuery: (query) => {
    set({ searchQuery: query });
    // Fetching is handled by the view's useDeferredValue + useEffect
    // to avoid race conditions during IME composition
  },

  setActiveFilter: (filter) => {
    set({ activeFilter: filter });
    get().fetchEntries();
  },

  setSelectedId: id => set({ selectedId: id }),

  // Fetch the first page (resets pagination). Called on mount, search, filter.
  fetchEntries: async () => {
    const { searchQuery, activeFilter } = get();
    const seq = ++fetchSeq;
    set({ isLoading: true });
    try {
      const query: ClipboardQuery = {
        search: searchQuery || null,
        content_type: activeFilter,
        pinned_only: false,
        limit: PAGE_SIZE,
        offset: 0,
      };
      const entries = await invoke<ClipboardEntry[]>("get_clipboard_entries", { query });
      // Drop this result if a newer fetch has started since (e.g. the user kept
      // typing) so a slow earlier query can't overwrite later results.
      if (seq !== fetchSeq) {
        return;
      }
      set({ entries, isLoading: false, hasMore: entries.length === PAGE_SIZE });
    } catch (e) {
      if (seq !== fetchSeq) {
        return;
      }
      console.error("Failed to fetch clipboard entries:", e);
      set({ isLoading: false });
      useToastStore.getState().add(parseAppError(e).message, "error");
    }
  },

  // Append the next page. Triggered when the list is scrolled near the bottom,
  // so the whole history is reachable rather than capped at the first page.
  loadMore: async () => {
    const { searchQuery, activeFilter, entries, hasMore, isLoadingMore, isLoading } = get();
    if (!hasMore || isLoadingMore || isLoading) {
      return;
    }
    // Tie this page to the current first-page fetch; if a new search/filter
    // resets the list mid-flight, discard the now-stale page.
    const seq = fetchSeq;
    set({ isLoadingMore: true });
    try {
      const query: ClipboardQuery = {
        search: searchQuery || null,
        content_type: activeFilter,
        pinned_only: false,
        limit: PAGE_SIZE,
        offset: entries.length,
      };
      const page = await invoke<ClipboardEntry[]>("get_clipboard_entries", { query });
      if (seq !== fetchSeq) {
        set({ isLoadingMore: false });
        return;
      }
      set((state) => {
        // Dedup by id in case the underlying order shifted between pages.
        const seen = new Set(state.entries.map(e => e.id));
        const fresh = page.filter(e => !seen.has(e.id));
        return {
          entries: [...state.entries, ...fresh],
          isLoadingMore: false,
          hasMore: page.length === PAGE_SIZE,
        };
      });
    } catch (e) {
      if (seq !== fetchSeq) {
        return;
      }
      console.error("Failed to load more clipboard entries:", e);
      set({ isLoadingMore: false });
    }
  },

  deleteEntry: async (id) => {
    try {
      await invoke("delete_clipboard_entry", { id });
      set((state) => {
        const oldEntries = state.entries;
        const idx = oldEntries.findIndex(e => e.id === id);
        const newEntries = oldEntries.filter(e => e.id !== id);

        // Auto-select next item (or previous if deleted the last one)
        let newSelectedId = state.selectedId;
        if (state.selectedId === id) {
          if (idx < newEntries.length) {
            newSelectedId = newEntries[idx]?.id ?? null;
          } else {
            newSelectedId = newEntries.at(-1)?.id ?? null;
          }
        }

        return { entries: newEntries, selectedId: newSelectedId };
      });
    } catch (e) {
      console.error("Failed to delete entry:", e);
      throw e;
    }
  },

  togglePin: async (id) => {
    try {
      const isPinned = await invoke<boolean>("toggle_pin_entry", { id });
      set((state) => {
        const newEntries = state.entries.map(e =>
          e.id === id ? { ...e, is_pinned: isPinned } : e,
        );
        // Re-sort: pinned first, then by accessed_at descending
        newEntries.sort((a, b) => {
          if (a.is_pinned !== b.is_pinned) {
            return a.is_pinned ? -1 : 1;
          }
          return (b.accessed_at ?? "").localeCompare(a.accessed_at ?? "");
        });
        return { entries: newEntries };
      });
    } catch (e) {
      console.error("Failed to toggle pin:", e);
      throw e;
    }
  },

  updateEntryContent: async (id, content) => {
    try {
      await invoke("update_entry_content", { id, content });
      const preview = content.replace(/\n/g, " ").slice(0, 200);
      set(state => ({
        entries: state.entries.map(e =>
          e.id === id ? { ...e, text_content: content, content_preview: preview } : e,
        ),
      }));
    } catch (e) {
      console.error("Failed to update entry content:", e);
      throw e;
    }
  },

  appendToClipboard: async (text) => {
    try {
      await invoke("append_to_clipboard", { text });
    } catch (e) {
      console.error("Failed to append to clipboard:", e);
      throw e;
    }
  },

  saveAsFile: async (content, defaultName) => {
    try {
      return await invoke<boolean>("save_entry_as_file", { content, defaultName });
    } catch (e) {
      console.error("Failed to save as file:", e);
      return false;
    }
  },

  pasteAndKeepWindow: async (text) => {
    try {
      await invoke("paste_and_keep_window", { text });
    } catch (e) {
      console.error("Failed to paste and keep window:", e);
      throw e;
    }
  },

  pasteImageAndKeepWindow: async (imagePath) => {
    try {
      await invoke("paste_image_and_keep_window", { imagePath });
    } catch (e) {
      console.error("Failed to paste image and keep window:", e);
      throw e;
    }
  },

  pasteFileAndKeepWindow: async (filePathsJson) => {
    try {
      await invoke("paste_file_and_keep_window", { filePathsJson });
    } catch (e) {
      console.error("Failed to paste file and keep window:", e);
      throw e;
    }
  },

  clearHistory: async () => {
    try {
      await invoke("clear_clipboard_history");
      set(state => ({
        entries: state.entries.filter(e => e.is_pinned),
        selectedId: null,
      }));
    } catch (e) {
      console.error("Failed to clear history:", e);
      throw e;
    }
  },

  pasteEntry: async (text) => {
    try {
      await invoke("paste_clipboard_entry", { text });
    } catch (e) {
      console.error("Failed to paste:", e);
      throw e;
    }
  },

  copyEntry: async (text) => {
    try {
      await invoke("copy_clipboard_entry", { text });
    } catch (e) {
      console.error("Failed to copy:", e);
      throw e;
    }
  },

  pasteAsPlainText: async (text) => {
    try {
      await invoke("paste_as_plain_text", { text });
    } catch (e) {
      console.error("Failed to paste as plain text:", e);
      throw e;
    }
  },

  pasteFileEntry: async (filePathsJson) => {
    try {
      await invoke("paste_file_entry", { filePathsJson });
    } catch (e) {
      console.error("Failed to paste file entry:", e);
      throw e;
    }
  },

  copyFileEntry: async (filePathsJson) => {
    try {
      await invoke("copy_file_entry", { filePathsJson });
    } catch (e) {
      console.error("Failed to copy file entry:", e);
      throw e;
    }
  },

  pasteImageEntry: async (imagePath) => {
    try {
      await invoke("paste_image_entry", { imagePath });
    } catch (e) {
      console.error("Failed to paste image entry:", e);
      throw e;
    }
  },

  copyImageEntry: async (imagePath) => {
    try {
      await invoke("copy_image_entry", { imagePath });
    } catch (e) {
      console.error("Failed to copy image entry:", e);
      throw e;
    }
  },

  addNewEntry: (entry) => {
    set((state) => {
      let newEntries: ClipboardEntry[];
      const existingIndex = state.entries.findIndex(e => e.id === entry.id);
      if (existingIndex !== -1) {
        // Merge with existing entry to preserve fields not in the event payload
        const merged = { ...state.entries[existingIndex], ...entry };
        newEntries = [...state.entries];
        newEntries[existingIndex] = merged;
      } else {
        newEntries = [entry, ...state.entries];
      }
      // Re-sort: pinned first, then by accessed_at descending
      newEntries.sort((a, b) => {
        if (a.is_pinned !== b.is_pinned) {
          return a.is_pinned ? -1 : 1;
        }
        return (b.accessed_at ?? "").localeCompare(a.accessed_at ?? "");
      });
      return { entries: newEntries };
    });
  },

  exportHistory: async () => {
    try {
      return await invoke<number>("export_clipboard_history");
    } catch (e) {
      console.error("Failed to export history:", e);
      throw e;
    }
  },

  importHistory: async () => {
    try {
      const count = await invoke<number>("import_clipboard_history");
      if (count > 0) {
        // Refresh entries after import
        await get().fetchEntries();
      }
      return count;
    } catch (e) {
      console.error("Failed to import history:", e);
      throw e;
    }
  },
}));
