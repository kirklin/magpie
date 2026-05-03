import {
  BookmarkPlus,
  Copy,
  Eraser,
  Pencil,
  Pin,
  PinOff,
  Share,
  Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface Action {
  id: string;
  label: string;
  icon: React.ReactNode;
  shortcut?: string[];
  danger?: boolean;
  onAction: () => void;
}

interface ActionPanelProps {
  isOpen: boolean;
  onClose: () => void;
  actions: Action[];
}

export function ActionPanel({ isOpen, onClose, actions }: ActionPanelProps) {
  const [search, setSearch] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const filtered = actions.filter(a =>
    a.label.toLowerCase().includes(search.toLowerCase()),
  );

  // Reset state when opened
  useEffect(() => {
    if (isOpen) {
      setSearch("");
      setSelectedIndex(0);
      // Focus after render
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Keyboard navigation
  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handler = (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          setSelectedIndex(i => Math.min(i + 1, filtered.length - 1));
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          setSelectedIndex(i => Math.max(i - 1, 0));
          break;
        }
        case "Enter": {
          e.preventDefault();
          if (filtered[selectedIndex]) {
            filtered[selectedIndex].onAction();
            onClose();
          }
          break;
        }
        case "Escape": {
          e.preventDefault();
          onClose();
          break;
        }
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, filtered, selectedIndex, onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <>
      {/* Transparent backdrop to catch clicks */}
      <div
        className="fixed inset-0 z-40"
        onClick={onClose}
      />

      {/* Panel - positioned bottom right above the actions button */}
      <div
        ref={panelRef}
        className="fixed right-4 bottom-14 w-[340px] bg-[#1E1E1E]/95 backdrop-blur-xl border border-white/10 rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.5)] z-50 flex flex-col-reverse overflow-hidden animate-scale-in"
        style={{ transformOrigin: "bottom right" }}
      >
        {/* Search - visually at bottom */}
        <div className="flex items-center px-3 h-11 bg-transparent shrink-0">
          <input
            ref={inputRef}
            className="flex-1 h-full bg-transparent border-none outline-none text-white/90 text-[13px] placeholder:text-white/40 font-sans"
            placeholder="Search..."
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setSelectedIndex(0);
            }}
            spellCheck={false}
          />
        </div>

        {/* Divider */}
        <div className="h-[1px] bg-white/10 w-full shrink-0" />

        {/* Actions list - visually above search */}
        <div className="max-h-[300px] overflow-y-auto p-1.5 flex flex-col gap-0.5 scrollbar-none">
          {filtered.length === 0
            ? (
                <div className="px-4 py-6 text-center text-white/40 text-sm">
                  No matching actions
                </div>
              )
            : (
                filtered.map((action, index) => (
                  <button
                    key={action.id}
                    className={`w-full flex items-center gap-3 px-2.5 py-2 rounded-lg text-left transition-colors ${
                      index === selectedIndex
                        ? "bg-white/10"
                        : "hover:bg-white/5"
                    } ${action.danger ? "text-red-400" : "text-white/90"}`}
                    onClick={() => {
                      action.onAction();
                      onClose();
                    }}
                    onMouseEnter={() => setSelectedIndex(index)}
                  >
                    <span className={`shrink-0 flex items-center justify-center w-5 h-5 rounded ${
                      index === selectedIndex
                        ? action.danger ? "text-red-400" : "text-white"
                        : "text-white/70"
                    }`}
                    >
                      {action.icon}
                    </span>
                    <span className="flex-1 text-[13px] font-medium">{action.label}</span>
                    {action.shortcut && (
                      <div className="flex items-center gap-1 opacity-60">
                        {action.shortcut.map((key, i) => (
                          <kbd
                            key={i}
                            className="flex items-center justify-center min-w-[20px] h-[22px] px-1 text-[11px] bg-white/10 rounded border border-white/10 font-sans shadow-sm"
                          >
                            {key}
                          </kbd>
                        ))}
                      </div>
                    )}
                  </button>
                ))
              )}
        </div>
      </div>
    </>
  );
}

// Helper to build actions for a clipboard entry
export function buildClipboardActions({
  hasEntry,
  isPinned,
  activeApp,
  onPaste,
  onPastePlain,
  onCopy,
  onTogglePin,
  onDelete,
  onRename,
  onSaveSnippet,
  onClearHistory,
}: {
  hasEntry: boolean;
  isPinned: boolean;
  activeApp: string;
  onPaste: () => void;
  onPastePlain: () => void;
  onCopy: () => void;
  onTogglePin: () => void;
  onDelete: () => void;
  onRename: () => void;
  onSaveSnippet: () => void;
  onClearHistory: () => void;
}): Action[] {
  const actions: Action[] = [];

  if (hasEntry) {
    actions.push(
      {
        id: "paste",
        label: `Paste to ${activeApp}`,
        icon: (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="text-blue-400">
            <path d="M16 7C16 7 19 4 22 4C22 7 19 10 16 10" />
            <path d="M8 7C8 7 5 4 2 4C2 7 5 10 8 10" />
            <path d="M12 20C12 20 6 14 6 9C6 6.79 7.79 5 10 5C11.2 5 12 5.5 12 5.5C12 5.5 12.8 5 14 5C16.21 5 18 6.79 18 9C18 14 12 20 12 20Z" />
          </svg>
        ),
        shortcut: ["↵"],
        onAction: onPaste,
      },
      {
        id: "copy",
        label: "Copy to Clipboard",
        icon: <Copy size={14} className="text-white/70" />,
        shortcut: ["⌘", "↵"],
        onAction: onCopy,
      },
      {
        id: "paste-plain",
        label: "Paste and Keep Window Open",
        icon: (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="text-blue-400">
            <path d="M16 7C16 7 19 4 22 4C22 7 19 10 16 10" />
            <path d="M8 7C8 7 5 4 2 4C2 7 5 10 8 10" />
            <path d="M12 20C12 20 6 14 6 9C6 6.79 7.79 5 10 5C11.2 5 12 5.5 12 5.5C12 5.5 12.8 5 14 5C16.21 5 18 6.79 18 9C18 14 12 20 12 20Z" />
          </svg>
        ),
        shortcut: ["⌥", "↵"],
        onAction: onPastePlain,
      },
      {
        id: "share",
        label: "Share...",
        icon: <Share size={14} className="text-white/70" />,
        shortcut: ["⇧", "⌘", "E"],
        onAction: () => {},
      },
      {
        id: "pin",
        label: isPinned ? "Unpin" : "Pin to Top",
        icon: isPinned ? <PinOff size={14} className="text-orange-400" /> : <Pin size={14} className="text-white/70" />,
        shortcut: ["⌘", "."],
        onAction: onTogglePin,
      },
      {
        id: "rename",
        label: "Rename",
        icon: <Pencil size={14} className="text-white/70" />,
        onAction: onRename,
      },
      {
        id: "save-snippet",
        label: "Save as Snippet",
        icon: <BookmarkPlus size={14} className="text-white/70" />,
        onAction: onSaveSnippet,
      },
      {
        id: "delete",
        label: "Delete",
        icon: <Trash2 size={14} />,
        shortcut: ["⌘", "⌫"],
        danger: true,
        onAction: onDelete,
      },
    );
  }

  actions.push({
    id: "clear",
    label: "Clear All History",
    icon: <Eraser size={14} />,
    danger: true,
    onAction: onClearHistory,
  });

  return actions;
}
