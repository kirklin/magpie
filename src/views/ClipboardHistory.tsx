import type { ClipboardEntry } from "../stores/clipboard";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Pin, Settings } from "lucide-react";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { ActionPanel, buildClipboardActions } from "../components/ActionPanel";
import { ClipboardItem } from "../components/ClipboardItem";
import { EmptyState } from "../components/EmptyState";
import { PreviewPanel } from "../components/PreviewPanel";
import { SearchBar } from "../components/SearchBar";
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
    pasteEntry,
    copyEntry,
    pasteAsPlainText,
    pasteFileEntry,
    copyFileEntry,
    deleteEntry,
    togglePin,
    renameEntry,
    clearHistory,
    activeFilter,
    setActiveFilter,
    addNewEntry,
    activeApp,
    setActiveApp,
  } = useClipboardStore();
  const { navigateTo } = useNavigationStore();

  const [isActionPanelOpen, setIsActionPanelOpen] = useState(false);
  const isComposingRef = useRef(false);

  const deferredSearch = useDeferredValue(searchQuery);

  // Selected entry
  const selectedEntry = useMemo(
    () => entries.find(e => e.id === selectedId) ?? null,
    [entries, selectedId],
  );

  // Group entries by date
  const groupedEntries = useMemo(() => groupByDate(entries), [entries]);

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

  // Paste helper - handles both text and file entries
  const handlePaste = useCallback(async () => {
    if (!selectedEntry) {
      return;
    }
    if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
      pasteFileEntry(selectedEntry.file_paths);
    } else if (selectedEntry.text_content) {
      pasteEntry(selectedEntry.text_content);
    }
  }, [selectedEntry, pasteEntry, pasteFileEntry]);

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't handle when action panel is open
      if (isActionPanelOpen) {
        return;
      }

      const target = e.target as HTMLElement;
      if (target.tagName === "INPUT" && e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Enter" && e.key !== "Escape") {
        return;
      }

      const currentIndex = entries.findIndex(entry => entry.id === selectedId);

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          const nextIndex = Math.min(currentIndex + 1, entries.length - 1);
          setSelectedId(entries[nextIndex]?.id ?? null);
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          const prevIndex = Math.max(currentIndex - 1, 0);
          setSelectedId(entries[prevIndex]?.id ?? null);
          break;
        }
        case "Enter": {
          e.preventDefault();
          handlePaste();
          break;
        }
        case "Backspace": {
          if (e.metaKey && selectedId) {
            e.preventDefault();
            deleteEntry(selectedId);
          }
          break;
        }
        case ".": {
          if (e.metaKey && selectedId) {
            e.preventDefault();
            togglePin(selectedId);
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
        case "Escape": {
          e.preventDefault();
          invoke("hide_window");
          break;
        }
      }
    },
    [entries, selectedId, selectedEntry, isActionPanelOpen, setSelectedId, handlePaste, deleteEntry, togglePin],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Auto-select first item
  useEffect(() => {
    if (entries.length > 0 && !selectedId) {
      setSelectedId(entries[0].id);
    }
  }, [entries, selectedId, setSelectedId]);

  // Build actions for selected entry
  const actions = useMemo(() => buildClipboardActions({
    hasEntry: !!selectedEntry,
    isPinned: selectedEntry?.is_pinned ?? false,
    activeApp,
    onPaste: handlePaste,
    onPastePlain: async () => {
      if (!selectedEntry) {
        return;
      }
      if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
        pasteFileEntry(selectedEntry.file_paths);
      } else if (selectedEntry.text_content) {
        pasteAsPlainText(selectedEntry.text_content);
      }
    },
    onCopy: () => {
      if (!selectedEntry) {
        return;
      }
      if (selectedEntry.content_type === "file" && selectedEntry.file_paths) {
        copyFileEntry(selectedEntry.file_paths);
      } else if (selectedEntry.text_content) {
        copyEntry(selectedEntry.text_content);
      }
    },
    onTogglePin: () => {
      if (selectedId) {
        togglePin(selectedId);
      }
    },
    onDelete: () => {
      if (selectedId) {
        deleteEntry(selectedId);
      }
    },
    onRename: () => {
      if (selectedId) {
        const name = window.prompt("Enter new name:", selectedEntry?.custom_name || "");
        if (name !== null) {
          renameEntry(selectedId, name);
        }
      }
    },
    onSaveSnippet: () => {
      // TODO: open snippet save dialog
    },
    onClearHistory: () => {
      if (window.confirm("Clear all clipboard history? Pinned items will be kept.")) {
        clearHistory();
      }
    },
  }), [selectedEntry, selectedId, activeApp, handlePaste, pasteAsPlainText, copyEntry, togglePin, deleteEntry, renameEntry, clearHistory]);

  return (
    <div className="flex flex-col h-full">
      <SearchBar
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

      <div className="flex flex-1 overflow-hidden">
        {/* Left panel — entry list */}
        <div className="w-[380px] shrink-0 overflow-y-auto border-r border-border">
          {entries.length === 0
            ? (
                <EmptyState
                  title={searchQuery ? "没有找到匹配的内容" : "剪贴板历史为空"}
                  subtitle={searchQuery ? "试试其他关键词" : "复制一些内容开始吧"}
                  icon={searchQuery ? "search" : "clipboard"}
                />
              )
            : (
                <div className="py-1">
                  {Array.from(groupedEntries.entries()).map(([dateLabel, groupEntries]) => (
                    <div key={dateLabel}>
                      <div className="px-4 pt-3 pb-1 text-[11px] font-medium text-text-tertiary uppercase tracking-wider flex items-center gap-1">
                        {dateLabel === "Pinned" && <Pin size={10} className="shrink-0" />}
                        {dateLabel}
                      </div>
                      {(groupEntries as ClipboardEntry[]).map(entry => (
                        <ClipboardItem
                          key={entry.id}
                          entry={entry}
                          isSelected={entry.id === selectedId}
                          onClick={() => setSelectedId(entry.id)}
                          onDoubleClick={async () => {
                            if (entry.content_type === "file" && entry.file_paths) {
                              pasteFileEntry(entry.file_paths);
                            } else if (entry.text_content) {
                              pasteEntry(entry.text_content);
                            }
                          }}
                        />
                      ))}
                    </div>
                  ))}
                </div>
              )}
        </div>

        {/* Right panel — preview */}
        <div className="flex-1 overflow-hidden bg-bg-secondary/50">
          <PreviewPanel entry={selectedEntry} />
        </div>
      </div>

      {/* Bottom action bar */}
      <div className="flex items-center justify-between px-4 h-11 border-t border-border bg-bg-primary shrink-0">
        <button
          className="flex items-center gap-2 text-text-primary hover:text-text-accent transition-colors rounded px-1.5 py-1 -ml-1.5"
          onClick={() => navigateTo("about")}
        >
          <img src="/logo.png" alt="Magpie" className="w-5 h-5 object-contain rounded shadow-sm" />
          <span className="text-[13px] font-medium">Magpie</span>
        </button>
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1.5 text-[13px] text-text-primary font-medium">
            Paste to
            {" "}
            {activeApp}
            <kbd className="inline-flex items-center justify-center min-w-[22px] h-5 px-1.5 text-[11px] text-text-tertiary bg-bg-tertiary rounded border border-border font-sans">↵</kbd>
          </span>
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
        actions={actions}
      />
    </div>
  );
}
