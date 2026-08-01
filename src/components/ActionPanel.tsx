import type React from "react";
import { useEffect, useRef, useState } from "react";
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

function ActionButton({ action, isSelected, onSelect, onClick, ref }: {
  action: Action;
  isSelected: boolean;
  onSelect: () => void;
  onClick: () => void;
  ref?: React.Ref<HTMLButtonElement>;
}) {
  return (
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
          {action.shortcut.map(sym => (
            <kbd
              key={sym}
              className="flex items-center justify-center min-w-[20px] h-[22px] px-1 text-[11px] bg-bg-hover rounded border border-border font-sans shadow-sm"
            >
              {sym}
            </kbd>
          ))}
        </div>
      )}
    </button>
  );
}

// --- ActionPanel Component ---

export function ActionPanel({ isOpen, onClose, groups }: ActionPanelProps) {
  const t = useT();
  const [search, setSearch] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const itemElementsRef = useRef<Map<number, HTMLButtonElement>>(new Map());

  // Flatten filtered actions for keyboard navigation
  const allActions = groups.flatMap(g => g.actions);
  const filtered = allActions.filter(a =>
    a.label.toLowerCase().includes(search.toLowerCase()),
  );

  // Reset the query and cursor when the panel opens. Derived during render so
  // the first painted frame is already clean, rather than briefly showing the
  // previous session's search text.
  const [wasOpen, setWasOpen] = useState(isOpen);
  if (isOpen !== wasOpen) {
    setWasOpen(isOpen);
    if (isOpen) {
      setSearch("");
      setSelectedIndex(0);
    }
  }

  // DOM-touching side effects stay in an effect.
  useEffect(() => {
    if (!isOpen) {
      return;
    }
    itemElementsRef.current.clear();
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [isOpen]);

  // Scroll selected item into view
  useEffect(() => {
    if (isOpen && filtered.length > 0) {
      const el = itemElementsRef.current.get(selectedIndex);
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
                          itemElementsRef.current.set(index, el);
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
                      // Groups may be unlabelled, so fall back to the id of the
                      // first surviving action — guaranteed present by the
                      // emptiness check above, and stable across re-renders in a
                      // way the array index is not.
                      <div key={group.label ?? groupActions[0].id}>
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
                                  itemElementsRef.current.set(currentIndex, el);
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
