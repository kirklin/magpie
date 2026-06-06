import { Check, ChevronDown, Code, FileText, Image, Layers, Link, Mail, Palette, Search, Type } from "lucide-react";
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  onCompositionChange?: (isComposing: boolean) => void;
  activeFilter: string | null;
  onFilterChange: (filter: string | null) => void;
  placeholder?: string;
}

export interface SearchBarRef {
  toggleFilter: () => void;
  focus: () => void;
}

const FILTER_OPTIONS: { value: string | null; label: string; icon: React.ReactNode }[] = [
  { value: null, label: "All Types", icon: <Layers size={14} /> },
  { value: "text", label: "Text", icon: <Type size={14} /> },
  { value: "image", label: "Images", icon: <Image size={14} /> },
  { value: "file", label: "Files", icon: <FileText size={14} /> },
  { value: "url", label: "Links", icon: <Link size={14} /> },
  { value: "color", label: "Colors", icon: <Palette size={14} /> },
  { value: "code", label: "Code", icon: <Code size={14} /> },
  { value: "email", label: "Emails", icon: <Mail size={14} /> },
];

export const SearchBar = forwardRef<SearchBarRef, SearchBarProps>(
  ({ value, onChange, onCompositionChange, activeFilter, onFilterChange, placeholder = "Type to filter entries…" }, ref) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const [isDropdownOpen, setIsDropdownOpen] = useState(false);
    const [highlightedIndex, setHighlightedIndex] = useState(-1);
    const itemRefs = useRef<Map<number, HTMLButtonElement>>(new Map());

    // Focus search input on mount and whenever the Tauri window gains focus
    useEffect(() => {
      inputRef.current?.focus();

      let unlisten: (() => void) | undefined;
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
        getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (focused) {
            requestAnimationFrame(() => inputRef.current?.focus());
          }
        }).then(fn => { unlisten = fn; });
      });

      return () => unlisten?.();
    }, []);

    // When dropdown opens, highlight the currently active filter
    useEffect(() => {
      if (isDropdownOpen) {
        const idx = FILTER_OPTIONS.findIndex(o => o.value === activeFilter);
        setHighlightedIndex(idx >= 0 ? idx : 0);
      }
    }, [isDropdownOpen, activeFilter]);

    // Scroll highlighted item into view
    useEffect(() => {
      if (isDropdownOpen && highlightedIndex >= 0) {
        itemRefs.current.get(highlightedIndex)?.scrollIntoView({ block: "nearest" });
      }
    }, [highlightedIndex, isDropdownOpen]);

    // Keyboard navigation for the dropdown
    const handleDropdownKeyDown = useCallback((e: KeyboardEvent) => {
      if (!isDropdownOpen) return;

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          e.stopPropagation();
          setHighlightedIndex(i => Math.min(i + 1, FILTER_OPTIONS.length - 1));
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          e.stopPropagation();
          setHighlightedIndex(i => Math.max(i - 1, 0));
          break;
        }
        case "Enter": {
          e.preventDefault();
          e.stopPropagation();
          if (highlightedIndex >= 0 && highlightedIndex < FILTER_OPTIONS.length) {
            onFilterChange(FILTER_OPTIONS[highlightedIndex].value);
            setIsDropdownOpen(false);
          }
          break;
        }
        case "Escape": {
          e.preventDefault();
          e.stopPropagation();
          setIsDropdownOpen(false);
          break;
        }
      }
    }, [isDropdownOpen, highlightedIndex, onFilterChange]);

    useEffect(() => {
      if (isDropdownOpen) {
        // Use capture phase to intercept before other handlers
        window.addEventListener("keydown", handleDropdownKeyDown, true);
        return () => window.removeEventListener("keydown", handleDropdownKeyDown, true);
      }
    }, [isDropdownOpen, handleDropdownKeyDown]);

    useImperativeHandle(ref, () => ({
      toggleFilter: () => setIsDropdownOpen(prev => !prev),
      focus: () => inputRef.current?.focus(),
    }));

    const currentFilterLabel = FILTER_OPTIONS.find(o => o.value === activeFilter)?.label || "All Types";

    return (
      <div className="drag-region flex items-center h-12 px-4 gap-3 border-b border-border shrink-0 relative">
        <div className="no-drag w-5 h-5 flex items-center justify-center text-text-tertiary">
          <Search size={16} strokeWidth={2.5} />
        </div>
        <input
          ref={inputRef}
          className="no-drag flex-1 h-full border-none outline-none bg-transparent text-text-primary text-[15px] font-sans caret-accent placeholder:text-text-tertiary"
          type="text"
          value={value}
          onChange={e => onChange(e.target.value)}
          onCompositionStart={() => onCompositionChange?.(true)}
          onCompositionEnd={(e) => {
            onCompositionChange?.(false);
            // Ensure the final composed value triggers a search
            onChange((e.target as HTMLInputElement).value);
          }}
          placeholder={placeholder}
          spellCheck={false}
          autoComplete="off"
        />
        {value && (
          <button
            className="no-drag w-5 h-5 flex items-center justify-center border-none bg-bg-tertiary text-text-secondary rounded-full text-[10px] transition-all duration-100 hover:bg-bg-active hover:text-text-primary shrink-0"
            onMouseDown={e => e.preventDefault()}
            onClick={() => onChange("")}
            title="清除"
          >
            ✕
          </button>
        )}

        {/* Custom Dropdown */}
        <div className="relative no-drag">
          <button
            className="flex items-center gap-2 bg-transparent text-text-secondary text-[13px] px-2.5 py-1.5 rounded-md hover:bg-bg-tertiary transition-colors"
            onMouseDown={e => e.preventDefault()}
            onClick={() => setIsDropdownOpen(!isDropdownOpen)}
          >
            {currentFilterLabel}
            <ChevronDown size={14} className="opacity-70" />
          </button>

          {isDropdownOpen && (
            <>
              <div
                className="fixed inset-0 z-10"
                onMouseDown={e => e.preventDefault()}
                onClick={() => setIsDropdownOpen(false)}
              />
              <div
                className="absolute right-0 top-full mt-1 w-44 bg-bg-secondary/95 backdrop-blur-xl border border-border rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.3)] p-1.5 z-20 animate-scale-in origin-top-right"
                onMouseDown={e => e.preventDefault()}
              >
                {FILTER_OPTIONS.map((option, index) => (
                  <button
                    key={option.value || "all"}
                    ref={el => { if (el) itemRefs.current.set(index, el); }}
                    className={`w-full flex items-center justify-between px-2.5 py-2 text-[13px] font-medium rounded-lg transition-colors ${
                      index === highlightedIndex
                        ? "bg-bg-hover text-text-primary"
                        : "text-text-secondary"
                    }`}
                    onClick={() => {
                      onFilterChange(option.value);
                      setIsDropdownOpen(false);
                    }}
                    onMouseEnter={() => setHighlightedIndex(index)}
                  >
                    <span className="flex items-center gap-2.5">
                      <span className={index === highlightedIndex ? "text-text-primary" : "text-text-tertiary"}>{option.icon}</span>
                      {option.label}
                    </span>
                    {activeFilter === option.value && <Check size={14} className="text-accent" />}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    );
  },
);

SearchBar.displayName = "SearchBar";
