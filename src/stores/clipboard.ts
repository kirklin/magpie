import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface ClipboardEntry {
  id: number;
  content_type: string;
  text_content: string | null;
  html_content: string | null;
  image_path: string | null;
  file_paths: string | null;
  source_app: string | null;
  source_app_name: string | null;
  custom_name: string | null;
  is_pinned: boolean;
  is_favorite: boolean;
  content_hash: string;
  content_preview: string | null;
  byte_size: number;
  created_at: string;
  accessed_at: string;
  access_count: number;
}

export interface ClipboardQuery {
  search: string | null;
  content_type: string | null;
  pinned_only: boolean;
  limit: number;
  offset: number;
}

interface ClipboardStore {
  entries: ClipboardEntry[];
  selectedId: number | null;
  searchQuery: string;
  activeFilter: string | null;
  isLoading: boolean;
  activeApp: string;

  setActiveApp: (app: string) => void;
  setSearchQuery: (query: string) => void;
  setActiveFilter: (filter: string | null) => void;
  setSelectedId: (id: number | null) => void;
  fetchEntries: () => Promise<void>;
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

  fetchEntries: async () => {
    const { searchQuery, activeFilter } = get();
    set({ isLoading: true });
    try {
      const query: ClipboardQuery = {
        search: searchQuery || null,
        content_type: activeFilter,
        pinned_only: false,
        limit: 200,
        offset: 0,
      };
      const entries = await invoke<ClipboardEntry[]>("get_clipboard_entries", { query });
      set({ entries, isLoading: false });
    } catch (e) {
      console.error("Failed to fetch clipboard entries:", e);
      set({ isLoading: false });
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
            newSelectedId = newEntries[newEntries.length - 1]?.id ?? null;
          }
        }

        return { entries: newEntries, selectedId: newSelectedId };
      });
    } catch (e) {
      console.error("Failed to delete entry:", e);
    }
  },

  togglePin: async (id) => {
    try {
      const isPinned = await invoke<boolean>("toggle_pin_entry", { id });
      set(state => {
        const newEntries = state.entries.map(e =>
          e.id === id ? { ...e, is_pinned: isPinned } : e,
        );
        // Re-sort: pinned first, then by accessed_at descending
        newEntries.sort((a, b) => {
          if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
          return (b.accessed_at ?? "").localeCompare(a.accessed_at ?? "");
        });
        return { entries: newEntries };
      });
    } catch (e) {
      console.error("Failed to toggle pin:", e);
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
    }
  },

  appendToClipboard: async (text) => {
    try {
      await invoke("append_to_clipboard", { text });
    } catch (e) {
      console.error("Failed to append to clipboard:", e);
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
    }
  },

  pasteImageAndKeepWindow: async (imagePath) => {
    try {
      await invoke("paste_image_and_keep_window", { imagePath });
    } catch (e) {
      console.error("Failed to paste image and keep window:", e);
    }
  },

  pasteFileAndKeepWindow: async (filePathsJson) => {
    try {
      await invoke("paste_file_and_keep_window", { filePathsJson });
    } catch (e) {
      console.error("Failed to paste file and keep window:", e);
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
    }
  },

  pasteEntry: async (text) => {
    try {
      await invoke("paste_clipboard_entry", { text });
    } catch (e) {
      console.error("Failed to paste:", e);
    }
  },

  copyEntry: async (text) => {
    try {
      await invoke("copy_clipboard_entry", { text });
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  },

  pasteAsPlainText: async (text) => {
    try {
      await invoke("paste_as_plain_text", { text });
    } catch (e) {
      console.error("Failed to paste as plain text:", e);
    }
  },

  pasteFileEntry: async (filePathsJson) => {
    try {
      await invoke("paste_file_entry", { filePathsJson });
    } catch (e) {
      console.error("Failed to paste file entry:", e);
    }
  },

  copyFileEntry: async (filePathsJson) => {
    try {
      await invoke("copy_file_entry", { filePathsJson });
    } catch (e) {
      console.error("Failed to copy file entry:", e);
    }
  },

  pasteImageEntry: async (imagePath) => {
    try {
      await invoke("paste_image_entry", { imagePath });
    } catch (e) {
      console.error("Failed to paste image entry:", e);
    }
  },

  copyImageEntry: async (imagePath) => {
    try {
      await invoke("copy_image_entry", { imagePath });
    } catch (e) {
      console.error("Failed to copy image entry:", e);
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
        if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
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
