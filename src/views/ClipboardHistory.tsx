import type { ClipboardEntry } from "../stores/clipboard";
import type { SearchBarRef } from "../components/SearchBar";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Filter, Pin, Settings } from "lucide-react";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { useCommandHold } from "../hooks/useCommandHold";
import { ActionPanel, buildClipboardActionGroups } from "../components/ActionPanel";
import { ClipboardItem } from "../components/ClipboardItem";
import { ConfirmModal } from "../components/ConfirmModal";
import { EditContentModal } from "../components/EditContentModal";
import { EmptyState } from "../components/EmptyState";
import { PreviewPanel } from "../components/PreviewPanel";
import { SearchBar } from "../components/SearchBar";
import { ToastContainer, useToastStore } from "../components/Toast";
import { useClipboardStore } from "../stores/clipboard";
import { useNavigationStore } from "../stores/navigation";
import { groupByDate } from "../utils/grouping";

export function ClipboardHistory() {
  const {
    entries,
    selectedId,
    searchQuery,
    setSearchQuery,
    setSelectedId,
    fetchEntries,
    loadMore,
    pasteEntry,
    copyEntry,
    pasteAsPlainText,
    pasteFileEntry,
    copyFileEntry,
    pasteImageEntry,
    copyImageEntry,
    deleteEntry,
    togglePin,
    clearHistory,
    activeFilter,
    setActiveFilter,
    addNewEntry,
    activeApp,
    setActiveApp,
    updateEntryContent,
    appendToClipboard,
    saveAsFile,
    pasteAndKeepWindow,
    pasteImageAndKeepWindow,
    pasteFileAndKeepWindow,
  } = useClipboardStore();
  const { navigateTo } = useNavigationStore();
  const toast = useToastStore();

  const [isActionPanelOpen, setIsActionPanelOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isConfirmClearOpen, setIsConfirmClearOpen] = useState(false);
  const isComposingRef = useRef(false);
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const visibleRange = useRef({ startIndex: 0, endIndex: 0 });
  const searchBarRef = useRef<SearchBarRef>(null);

  // Command-hold quick-paste feature
  const isCommandHeld = useCommandHold(300);

  const deferredSearch = useDeferredValue(searchQuery);

  // Selected entry
  const selectedEntry = useMemo(
    () => entries.find(e => e.id === selectedId) ?? null,
    [entries, selectedId],
  );

  // Group entries by date
  const groupedEntries = useMemo(() => groupByDate(entries), [entries]);

  // Flatten the date groups into a single row list (header rows + item rows)
  // so the list can be virtualized. Item rows carry their flat index (position
  // among entries) for the ⌘-hold quick-paste badge.
  type Row =
    | { kind: "header"; label: string }
    | { kind: "item"; entry: ClipboardEntry; index: number };
  const rows = useMemo<Row[]>(() => {
    const out: Row[] = [];
    let itemIndex = 0;
    for (const [label, groupEntries] of groupedEntries) {
      out.push({ kind: "header", label });
      for (const entry of groupEntries as ClipboardEntry[]) {
        out.push({ kind: "item", entry, index: itemIndex });
        itemIndex += 1;
      }
    }
    return out;
  }, [groupedEntries]);

  // Fetch entries on mount and when search changes (but not during IME composition)
  useEffect(() => {
    if (!isComposingRef.current) {
      fetchEntries();
    }
  }, [deferredSearch]);

  // Listen for clipboard changes from Rust
  useEffect(() => {
    const unlisten = listen<ClipboardEntry>("clipboard://changed", (event) => {
      addNewEntry(event.payload);
    });

    const unlistenActiveApp = listen<string>("active-app-changed", (event) => {
      setActiveApp(event.payload);
    });

    return () => {
      unlisten.then(fn => fn());
      unlistenActiveApp.then(fn => fn());
    };
  }, [addNewEntry, setActiveApp]);

  // --- Action callbacks ---

  const handlePaste = useCallback(async () => {
    if (!selectedEntry) return;
    try {
      if (selectedEntry.content_type === "image" && selectedEntry.image_path) {
        await pasteImageEntry(selectedEntry.image_path);
      } else if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
        await pasteFileEntry(selectedEntry.file_paths);
      } else if (selectedEntry.text_content) {
        await pasteEntry(selectedEntry.text_content);
      }
    } catch {
      toast.add("Paste failed");
    }
  }, [selectedEntry, pasteEntry, pasteFileEntry, pasteImageEntry, toast]);

  const handleCopy = useCallback(async () => {
    if (!selectedEntry) return;
    try {
      if (selectedEntry.content_type === "image" && selectedEntry.image_path) {
        await copyImageEntry(selectedEntry.image_path);
      } else if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
        await copyFileEntry(selectedEntry.file_paths);
      } else if (selectedEntry.text_content) {
        await copyEntry(selectedEntry.text_content);
      } else {
        return;
      }
      toast.add("Copied to clipboard");
    } catch {
      toast.add("Copy failed");
    }
  }, [selectedEntry, copyEntry, copyFileEntry, copyImageEntry, toast]);

  const handlePastePlainText = useCallback(async () => {
    if (!selectedEntry?.text_content) return;
    try {
      await pasteAsPlainText(selectedEntry.text_content);
    } catch {
      toast.add("Paste failed");
    }
  }, [selectedEntry, pasteAsPlainText, toast]);

  const handlePasteKeepWindow = useCallback(async () => {
    if (!selectedEntry) return;
    try {
      if (selectedEntry.content_type === "image" && selectedEntry.image_path) {
        await pasteImageAndKeepWindow(selectedEntry.image_path);
      } else if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
        await pasteFileAndKeepWindow(selectedEntry.file_paths);
      } else if (selectedEntry.text_content) {
        await pasteAndKeepWindow(selectedEntry.text_content);
      } else {
        return;
      }
      toast.add("Pasted (window kept open)");
    } catch {
      toast.add("Paste failed");
    }
  }, [selectedEntry, pasteAndKeepWindow, pasteImageAndKeepWindow, pasteFileAndKeepWindow, toast]);

  const handleOpenUrl = useCallback(() => {
    if (!selectedEntry?.text_content) return;
    window.open(selectedEntry.text_content, "_blank");
  }, [selectedEntry]);

  const handleAppendToClipboard = useCallback(() => {
    if (!selectedEntry?.text_content) return;
    appendToClipboard(selectedEntry.text_content);
    toast.add("Appended to clipboard");
  }, [selectedEntry, appendToClipboard, toast]);

  const handleEditContent = useCallback(() => {
    if (!selectedEntry?.text_content) return;
    setIsEditModalOpen(true);
  }, [selectedEntry]);

  const handleSaveEditContent = useCallback((content: string) => {
    if (!selectedId) return;
    updateEntryContent(selectedId, content);
    toast.add("Content updated");
  }, [selectedId, updateEntryContent, toast]);

  const handleTogglePin = useCallback(() => {
    if (!selectedId) return;
    const wasPinned = selectedEntry?.is_pinned;
    togglePin(selectedId);
    toast.add(wasPinned ? "Unpinned" : "Pinned to top");
  }, [selectedId, selectedEntry, togglePin, toast]);

  const handleSaveAsFile = useCallback(() => {
    if (!selectedEntry) return;
    const content = selectedEntry.text_content ?? "";
    // Derive a default filename from the content type
    const ext = selectedEntry.content_type === "code" ? "txt"
      : selectedEntry.content_type === "url" ? "url"
        : "txt";
    const preview = (selectedEntry.content_preview ?? "clipboard")
      .slice(0, 30)
      .replace(/[^a-zA-Z0-9\u4e00-\u9fff]/g, "_");
    const defaultName = `${preview}.${ext}`;
    saveAsFile(content, defaultName);
  }, [selectedEntry, saveAsFile]);

  const handleDelete = useCallback(() => {
    if (selectedId) {
      deleteEntry(selectedId);
      toast.add("Deleted");
    }
  }, [selectedId, deleteEntry, toast]);

  const handleClearHistory = useCallback(() => {
    setIsConfirmClearOpen(true);
  }, []);

  const handleConfirmClear = useCallback(() => {
    clearHistory();
    setIsConfirmClearOpen(false);
    toast.add("History cleared");
  }, [clearHistory, toast]);

  // --- Keyboard navigation ---

  // Quick-paste helper: paste the Nth visible entry (0-indexed)
  const quickPasteByIndex = useCallback(async (index: number) => {
    const entry = entries[index];
    if (!entry) return;
    try {
      if (entry.content_type === "image" && entry.image_path) {
        await pasteImageEntry(entry.image_path);
      } else if (entry.content_type === "file" && entry.file_paths) {
        await pasteFileEntry(entry.file_paths);
      } else if (entry.text_content) {
        await pasteEntry(entry.text_content);
      }
    } catch {
      toast.add("Paste failed");
    }
  }, [entries, pasteEntry, pasteFileEntry, pasteImageEntry, toast]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't handle when modals are open
      if (isActionPanelOpen || isEditModalOpen || isConfirmClearOpen) return;

      // ⌘ + number (1-9) → quick-paste the Nth item
      if (e.metaKey && !e.altKey && !e.shiftKey && !e.ctrlKey) {
        const num = Number(e.key);
        if (num >= 1 && num <= 9) {
          e.preventDefault();
          quickPasteByIndex(num - 1);
          return;
        }
      }

      const target = e.target as HTMLElement;
      // When typing in the search input, only handle navigation keys and ⌘ shortcuts
      if (target.tagName === "INPUT" && !e.metaKey && !e.ctrlKey && e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Enter" && e.key !== "Escape") {
        return;
      }

      const currentIndex = entries.findIndex(entry => entry.id === selectedId);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          const nextIndex = Math.min(currentIndex + 1, entries.length - 1);
          setSelectedId(entries[nextIndex]?.id ?? null);
          // Page in more when navigating near the end of the loaded set.
          if (nextIndex >= entries.length - 5) loadMore();
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          const prevIndex = Math.max(currentIndex - 1, 0);
          setSelectedId(entries[prevIndex]?.id ?? null);
          break;
        }
        case "Enter": {
          if (e.shiftKey) {
            // Shift+Enter → Paste as plain text
            e.preventDefault();
            handlePastePlainText();
          } else if (e.altKey) {
            // Alt+Enter → Paste and keep window
            e.preventDefault();
            handlePasteKeepWindow();
          } else if (e.metaKey) {
            // Cmd+Enter → Copy to clipboard
            e.preventDefault();
            handleCopy();
          } else {
            // Enter → Paste
            e.preventDefault();
            handlePaste();
          }
          break;
        }
        case "Backspace": {
          if (e.metaKey && e.shiftKey) {
            e.preventDefault();
            handleClearHistory();
          } else if (e.metaKey && selectedId) {
            e.preventDefault();
            handleDelete();
          }
          break;
        }
        case ".": {
          if (e.metaKey && selectedId) {
            e.preventDefault();
            handleTogglePin();
          }
          break;
        }
        case "e": {
          if (e.metaKey && selectedEntry?.text_content) {
            e.preventDefault();
            handleEditContent();
          }
          break;
        }
        case "o": {
          if (e.metaKey && selectedEntry?.content_type === "url") {
            e.preventDefault();
            handleOpenUrl();
          }
          break;
        }
        case "c": {
          if (e.metaKey && e.altKey && selectedEntry?.text_content) {
            e.preventDefault();
            handleAppendToClipboard();
          }
          break;
        }
        case "s": {
          if (e.metaKey && selectedEntry) {
            e.preventDefault();
            handleSaveAsFile();
          }
          break;
        }
        case "k": {
          if (e.metaKey) {
            e.preventDefault();
            setIsActionPanelOpen(true);
          }
          break;
        }
        case ",": {
          if (e.metaKey) {
            e.preventDefault();
            navigateTo("settings");
          }
          break;
        }
        case "f": {
          if (e.metaKey) {
            e.preventDefault();
            searchBarRef.current?.toggleFilter();
          }
          break;
        }
        case "Escape": {
          e.preventDefault();
          invoke("hide_window");
          break;
        }
      }
    },
    [
      entries, selectedId, selectedEntry, isActionPanelOpen, isEditModalOpen, isConfirmClearOpen,
      setSelectedId, handlePaste, handleCopy, handlePastePlainText, handlePasteKeepWindow,
      handleDelete, handleTogglePin, handleEditContent, handleOpenUrl,
      handleAppendToClipboard, handleSaveAsFile, handleClearHistory, navigateTo,
      quickPasteByIndex, loadMore,
    ],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Refocus search input when overlays close
  useEffect(() => {
    if (!isActionPanelOpen && !isEditModalOpen && !isConfirmClearOpen) {
      requestAnimationFrame(() => searchBarRef.current?.focus());
    }
  }, [isActionPanelOpen, isEditModalOpen, isConfirmClearOpen]);

  // Auto-select first item
  useEffect(() => {
    if (entries.length > 0 && !selectedId) {
      setSelectedId(entries[0].id);
    }
  }, [entries, selectedId, setSelectedId]);

  // Scroll the keyboard-selected row into view. We compare against the rendered
  // range (tracked via Virtuoso's rangeChanged, with no overscan so rendered ==
  // visible) and only scroll when the row is above or below it — scrolling to
  // `start`/`end` so it lands fully in view. Depends only on selectedId so
  // appending more pages doesn't yank the scroll.
  useEffect(() => {
    if (!selectedId) return;
    const rowIndex = rows.findIndex(r => r.kind === "item" && r.entry.id === selectedId);
    if (rowIndex < 0) return;
    const { startIndex, endIndex } = visibleRange.current;
    // Small margin so the selected row lands fully in view at the edge without
    // being cut, but not so large that a whole extra row shows past it (which
    // makes the selected row look like the second-to-last).
    const SCROLL_MARGIN = 24;
    if (rowIndex <= startIndex) {
      virtuosoRef.current?.scrollToIndex({ index: rowIndex, align: "start", offset: -SCROLL_MARGIN });
    } else if (rowIndex >= endIndex) {
      virtuosoRef.current?.scrollToIndex({ index: rowIndex, align: "end", offset: SCROLL_MARGIN });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  // Build grouped actions for the action panel
  const actionGroups = useMemo(() => buildClipboardActionGroups({
    hasEntry: !!selectedEntry,
    entry: selectedEntry ? {
      content_type: selectedEntry.content_type,
      text_content: selectedEntry.text_content,
      is_pinned: selectedEntry.is_pinned,
    } : null,
    activeApp,
    onPaste: handlePaste,
    onCopy: handleCopy,
    onPastePlainText: handlePastePlainText,
    onPasteKeepWindow: handlePasteKeepWindow,
    onOpenUrl: handleOpenUrl,
    onAppendToClipboard: handleAppendToClipboard,
    onEditContent: handleEditContent,
    onTogglePin: handleTogglePin,
    onSaveAsFile: handleSaveAsFile,
    onDelete: handleDelete,
    onClearHistory: handleClearHistory,
  }), [
    selectedEntry, activeApp,
    handlePaste, handleCopy, handlePastePlainText, handlePasteKeepWindow,
    handleOpenUrl, handleAppendToClipboard, handleEditContent,
    handleTogglePin, handleSaveAsFile, handleDelete, handleClearHistory,
  ]);

  return (
    <div className="flex flex-col h-full">
      <SearchBar
        ref={searchBarRef}
        value={searchQuery}
        onChange={setSearchQuery}
        onCompositionChange={(composing) => {
          isComposingRef.current = composing;
          if (!composing) {
            // Composition ended, trigger fetch with final value
            fetchEntries();
          }
        }}
        activeFilter={activeFilter}
        onFilterChange={setActiveFilter}
      />

      <div className="flex flex-1 overflow-hidden" onMouseDown={e => e.preventDefault()}>
        {/* Left panel — entry list */}
        <div className="relative w-[380px] shrink-0 border-r border-border">
          {/* Quick-paste gradient mask — single overlay for entire list right edge */}
          {isCommandHeld && <div className="quick-paste-mask" />}
          {entries.length === 0
            ? (
                <EmptyState
                  title={searchQuery ? "没有找到匹配的内容" : "剪贴板历史为空"}
                  subtitle={searchQuery ? "试试其他关键词" : "复制一些内容开始吧"}
                  icon={searchQuery ? "search" : "clipboard"}
                />
              )
            : (
                <Virtuoso
                  ref={virtuosoRef}
                  className="h-full"
                  data={rows}
                  endReached={() => loadMore()}
                  rangeChanged={(range) => {
                    visibleRange.current = range;
                  }}
                  computeItemKey={(_, row) =>
                    row.kind === "header" ? `h:${row.label}` : `i:${row.entry.id}`}
                  itemContent={(_, row) => {
                    if (row.kind === "header") {
                      return (
                        <div className="px-4 pt-3 pb-1 text-[11px] font-medium text-text-tertiary uppercase tracking-wider flex items-center gap-1">
                          {row.label === "Pinned" && <Pin size={10} className="shrink-0" />}
                          {row.label}
                        </div>
                      );
                    }
                    const { entry, index } = row;
                    const qpIndex = isCommandHeld && index < 9 ? index + 1 : undefined;
                    return (
                      <ClipboardItem
                        entry={entry}
                        isSelected={entry.id === selectedId}
                        quickPasteIndex={qpIndex}
                        onClick={() => setSelectedId(entry.id)}
                        onDoubleClick={async () => {
                          try {
                            if (entry.content_type === "image" && entry.image_path) {
                              await pasteImageEntry(entry.image_path);
                            } else if (entry.content_type === "file" && entry.file_paths) {
                              await pasteFileEntry(entry.file_paths);
                            } else if (entry.text_content) {
                              await pasteEntry(entry.text_content);
                            }
                          } catch {
                            toast.add("Paste failed");
                          }
                        }}
                      />
                    );
                  }}
                />
              )}
        </div>

        {/* Right panel — preview */}
        <div className="flex-1 overflow-hidden bg-bg-secondary/50">
          <PreviewPanel entry={selectedEntry} />
        </div>
      </div>

      {/* Bottom action bar */}
      <div className="flex items-center justify-between px-4 h-11 border-t border-border bg-bg-primary shrink-0" onMouseDown={e => e.preventDefault()}>
        <button
          className="flex items-center gap-2 text-text-primary hover:text-text-accent transition-colors rounded px-1.5 py-1 -ml-1.5"
          onClick={() => navigateTo("about")}
        >
          <img src="/logo.png" alt="Magpie" className="w-5 h-5 object-contain rounded shadow-sm" />
          <span className="text-[13px] font-medium">Magpie</span>
        </button>
        <div className="flex items-center gap-3">
          <button
            className="flex items-center gap-1.5 text-[13px] text-text-primary font-medium hover:text-text-accent transition-colors"
            onClick={handlePaste}
          >
            Paste to
            {" "}
            {activeApp}
            <kbd className="inline-flex items-center justify-center min-w-[22px] h-5 px-1.5 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">↵</kbd>
          </button>
          <div className="w-[1px] h-3.5 bg-border mx-1"></div>
          <button
            className="flex items-center gap-1.5 text-[13px] text-text-secondary hover:text-text-primary transition-colors"
            onClick={() => setIsActionPanelOpen(true)}
          >
            Actions
            <div className="flex items-center gap-0.5">
              <kbd className="inline-flex items-center justify-center min-w-[20px] h-5 px-1 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">⌘</kbd>
              <kbd className="inline-flex items-center justify-center min-w-[20px] h-5 px-1 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">K</kbd>
            </div>
          </button>
          <div className="w-[1px] h-3.5 bg-border mx-1"></div>
          <button
            className="flex items-center gap-1.5 text-[13px] text-text-secondary hover:text-text-primary transition-colors"
            onClick={() => searchBarRef.current?.toggleFilter()}
          >
            <Filter className="w-3.5 h-3.5" />
            Filter
            <kbd className="inline-flex items-center justify-center min-w-[20px] h-5 px-1 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">⌘</kbd>
            <kbd className="inline-flex items-center justify-center min-w-[20px] h-5 px-1 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">F</kbd>
          </button>
          <div className="w-[1px] h-3.5 bg-border mx-1"></div>
          <button
            className="flex items-center justify-center w-5 h-5 text-text-secondary hover:text-text-primary transition-colors"
            onClick={() => navigateTo("settings")}
            title="Settings"
          >
            <Settings className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Action Panel */}
      <ActionPanel
        isOpen={isActionPanelOpen}
        onClose={() => setIsActionPanelOpen(false)}
        groups={actionGroups}
      />

      {/* Edit Content Modal */}
      <EditContentModal
        isOpen={isEditModalOpen}
        initialContent={selectedEntry?.text_content ?? ""}
        onSave={handleSaveEditContent}
        onClose={() => setIsEditModalOpen(false)}
      />

      {/* Confirm Clear History Modal */}
      <ConfirmModal
        isOpen={isConfirmClearOpen}
        title="Clear All History"
        message="All clipboard history will be permanently deleted. Pinned items will be kept."
        confirmLabel="Clear All"
        onConfirm={handleConfirmClear}
        onCancel={() => setIsConfirmClearOpen(false)}
      />

      {/* Toast Notifications */}
      <ToastContainer />
    </div>
  );
}
