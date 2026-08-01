import type { StringKey } from "../i18n";
import type { Action, ActionGroup } from "./ActionPanel";
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

// Split out of ActionPanel.tsx so that file exports only components — mixing a
// plain function in there breaks Fast Refresh for the whole module.

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
