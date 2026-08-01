import type { StringKey } from "../i18n";
import {
  ClipboardPaste,
  Copy,
  Eraser,
  ExternalLink,
  FileDown,
  ListPlus,
  Pencil,
  Pin,
  PinOff,
  Trash2,
  Type,
} from "lucide-react";
import { forwardRef, useEffect, useRef, useState } from "react";
import { useT } from "../i18n";

// --- ActionButton Component ---

// --- Types ---

export interface Action {
  id: string;
  label: string;
  icon: React.ReactNode;
  shortcut?: string[];
  danger?: boolean;
  onAction: () => void;
}

export interface ActionGroup {
  label?: string;
  actions: Action[];
}

interface ActionPanelProps {
  isOpen: boolean;
  onClose: () => void;
  groups: ActionGroup[];
}

const ActionButton = forwardRef<
  HTMLButtonElement,
  {
    action: Action;
    isSelected: boolean;
    onSelect: () => void;
    onClick: () => void;
  }
>(({ action, isSelected, onSelect, onClick }, ref) => (
  <button
    ref={ref}
    className={`w-full flex items-center gap-3 px-2.5 py-2 rounded-lg text-left transition-colors ${
      isSelected
        ? "bg-bg-hover"
        : "hover:bg-bg-hover/50"
    } ${action.danger ? "text-red-400" : "text-text-primary"}`}
    onClick={onClick}
    onMouseEnter={onSelect}
  >
    <span className={`shrink-0 flex items-center justify-center w-5 h-5 rounded ${
      isSelected
        ? action.danger ? "text-red-400" : "text-text-primary"
        : "text-text-secondary"
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
            className="flex items-center justify-center min-w-[20px] h-[22px] px-1 text-[11px] bg-bg-hover rounded border border-border font-sans shadow-sm"
          >
            {key}
          </kbd>
        ))}
      </div>
    )}
  </button>
));

ActionButton.displayName = "ActionButton";

// --- ActionPanel Component ---

export function ActionPanel({ isOpen, onClose, groups }: ActionPanelProps) {
  const t = useT();
  const [search, setSearch] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Map<number, HTMLButtonElement>>(new Map());

  // Flatten filtered actions for keyboard navigation
  const allActions = groups.flatMap(g => g.actions);
  const filtered = allActions.filter(a =>
    a.label.toLowerCase().includes(search.toLowerCase()),
  );

  // Reset state when opened
  useEffect(() => {
    if (isOpen) {
      setSearch("");
      setSelectedIndex(0);
      itemRefs.current.clear();
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Scroll selected item into view
  useEffect(() => {
    if (isOpen && filtered.length > 0) {
      const el = itemRefs.current.get(selectedIndex);
      el?.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex, isOpen, filtered.length]);

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

  // When searching, flatten all actions (ignore groups)
  const isSearching = search.length > 0;

  // Build a flat index for matching filtered → selectedIndex
  let flatIndex = 0;

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
        className="fixed right-4 bottom-14 w-[340px] bg-bg-secondary/95 backdrop-blur-xl border border-border rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.3)] z-50 flex flex-col-reverse overflow-hidden animate-scale-in"
        style={{ transformOrigin: "bottom right" }}
      >
        {/* Search - visually at bottom */}
        <div className="flex items-center px-3 h-11 bg-transparent shrink-0">
          <input
            ref={inputRef}
            className="flex-1 h-full bg-transparent border-none outline-none text-text-primary text-[13px] placeholder:text-text-tertiary font-sans"
            placeholder={t("action.search_placeholder")}
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setSelectedIndex(0);
            }}
            spellCheck={false}
          />
        </div>

        {/* Divider */}
        <div className="h-[1px] bg-border w-full shrink-0" />

        {/* Actions list - visually above search */}
        <div className="max-h-[340px] overflow-y-auto p-1.5 flex flex-col scrollbar-none">
          {filtered.length === 0
            ? (
                <div className="px-4 py-6 text-center text-text-tertiary text-sm">
                  {t("action.no_match")}
                </div>
              )
            : isSearching
              ? (
                  /* Flat list when searching */
                  filtered.map((action, index) => (
                    <ActionButton
                      key={action.id}
                      action={action}
                      isSelected={index === selectedIndex}
                      onSelect={() => setSelectedIndex(index)}
                      onClick={() => {
                        action.onAction();
                        onClose();
                      }}
                      ref={(el) => {
                        if (el) {
                          itemRefs.current.set(index, el);
                        }
                      }}
                    />
                  ))
                )
              : (
                  /* Grouped list when not searching */
                  groups.map((group, groupIdx) => {
                    const groupActions = group.actions.filter(a =>
                      a.label.toLowerCase().includes(search.toLowerCase()),
                    );
                    if (groupActions.length === 0) {
                      return null;
                    }

                    return (
                      <div key={group.label ?? groupIdx}>
                        {/* Separator between groups (not before first) */}
                        {groupIdx > 0 && (
                          <div className="h-[1px] bg-border mx-1.5 my-1" />
                        )}
                        {groupActions.map((action) => {
                          const currentIndex = flatIndex++;
                          return (
                            <ActionButton
                              key={action.id}
                              action={action}
                              isSelected={currentIndex === selectedIndex}
                              onSelect={() => setSelectedIndex(currentIndex)}
                              onClick={() => {
                                action.onAction();
                                onClose();
                              }}
                              ref={(el) => {
                                if (el) {
                                  itemRefs.current.set(currentIndex, el);
                                }
                              }}
                            />
                          );
                        })}
                      </div>
                    );
                  })
                )}
        </div>
      </div>
    </>
  );
}

// --- Action Builder ---

export interface BuildActionsConfig {
  /** Whether an entry is selected */
  hasEntry: boolean;
  /** The selected entry (null if none) */
  entry: {
    content_type: string;
    text_content: string | null;
    is_pinned: boolean;
  } | null;
  /** Name of the active app to paste to */
  activeApp: string;
  /** Translator (from useT) for action labels */
  t: (key: StringKey, params?: Record<string, string | number>) => string;
  /** Callbacks */
  onPaste: () => void;
  onCopy: () => void;
  onPastePlainText: () => void;
  onPasteKeepWindow: () => void;
  onOpenUrl: () => void;
  onAppendToClipboard: () => void;
  onEditContent: () => void;
  onTogglePin: () => void;
  onSaveAsFile: () => void;
  onDelete: () => void;
  onClearHistory: () => void;
}

/**
 * Builds the action groups for the clipboard action panel.
 * Groups are: Paste, Manage, Danger.
 * Context-aware: shows "Open in Browser" only for URL entries.
 */
export function buildClipboardActionGroups(config: BuildActionsConfig): ActionGroup[] {
  const { t } = config;
  const groups: ActionGroup[] = [];

  if (config.hasEntry && config.entry) {
    const isUrl = config.entry.content_type === "url";
    const isText = config.entry.text_content !== null;
    const isPinned = config.entry.is_pinned;

    // --- Group 1: Paste operations ---
    const pasteActions: Action[] = [
      {
        id: "paste",
        label: t("action.paste_to", { app: config.activeApp }),
        icon: <img src="/logo.png" alt="Magpie" className="w-[14px] h-[14px] object-contain rounded-[3px]" />,
        shortcut: ["↵"],
        onAction: config.onPaste,
      },
      {
        id: "copy",
        label: t("action.copy"),
        icon: <Copy size={14} className="text-text-secondary" />,
        shortcut: ["⌘", "↵"],
        onAction: config.onCopy,
      },
    ];

    if (isText) {
      pasteActions.push({
        id: "paste-plain",
        label: t("action.paste_plain"),
        icon: <Type size={14} className="text-text-secondary" />,
        shortcut: ["⇧", "↵"],
        onAction: config.onPastePlainText,
      });
    }

    pasteActions.push({
      id: "paste-keep-window",
      label: t("action.paste_keep"),
      icon: <ClipboardPaste size={14} className="text-text-secondary" />,
      shortcut: ["⌥", "↵"],
      onAction: config.onPasteKeepWindow,
    });

    groups.push({ actions: pasteActions });

    // --- Group 2: Management ---
    const manageActions: Action[] = [];

    if (isUrl) {
      manageActions.push({
        id: "open-url",
        label: t("action.open_browser"),
        icon: <ExternalLink size={14} className="text-text-secondary" />,
        shortcut: ["⌘", "O"],
        onAction: config.onOpenUrl,
      });
    }

    if (isText) {
      manageActions.push({
        id: "append",
        label: t("action.append"),
        icon: <ListPlus size={14} className="text-text-secondary" />,
        shortcut: ["⌘", "⌥", "C"],
        onAction: config.onAppendToClipboard,
      });
    }

    if (isText) {
      manageActions.push({
        id: "edit-content",
        label: t("action.edit"),
        icon: <Pencil size={14} className="text-text-secondary" />,
        shortcut: ["⌘", "E"],
        onAction: config.onEditContent,
      });
    }

    manageActions.push({
      id: "pin",
      label: isPinned ? t("action.unpin") : t("action.pin"),
      icon: isPinned ? <PinOff size={14} className="text-text-secondary" /> : <Pin size={14} className="text-text-secondary" />,
      shortcut: ["⌘", "."],
      onAction: config.onTogglePin,
    });

    manageActions.push({
      id: "save-as-file",
      label: t("action.save_file"),
      icon: <FileDown size={14} className="text-text-secondary" />,
      shortcut: ["⌘", "S"],
      onAction: config.onSaveAsFile,
    });

    groups.push({ actions: manageActions });

    // --- Group 3: Danger zone ---
    groups.push({
      actions: [
        {
          id: "delete",
          label: t("action.delete"),
          icon: <Trash2 size={14} />,
          shortcut: ["⌘", "⌫"],
          danger: true,
          onAction: config.onDelete,
        },
      ],
    });
  }

  // Always show clear history
  groups.push({
    actions: [
      {
        id: "clear",
        label: t("action.clear_all"),
        icon: <Eraser size={14} />,
        shortcut: ["⇧", "⌘", "⌫"],
        danger: true,
        onAction: config.onClearHistory,
      },
    ],
  });

  return groups;
}
