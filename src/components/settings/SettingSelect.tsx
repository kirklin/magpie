import { Check, ChevronDown } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface SettingSelectProps {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
}

const MENU_WIDTH = 176;
const MENU_GAP = 4;
/** Row height plus the menu's own padding, for deciding whether it fits below. */
const ROW_HEIGHT = 36;
const MENU_PADDING = 12;

/**
 * A setting chosen from a short list. The menu is drawn by the page itself:
 * a native `<select>` opens a separate popup window on Linux and Windows,
 * and the focus it takes from the main window would hide Magpie.
 */
export function SettingSelect({ label, value, options, onChange }: SettingSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const current = options.find(o => o.value === value);

  const open = () => {
    if (buttonRef.current) {
      setAnchorRect(buttonRef.current.getBoundingClientRect());
    }
    setHighlightedIndex(Math.max(0, options.findIndex(o => o.value === value)));
    setIsOpen(true);
  };

  const close = useCallback(() => setIsOpen(false), []);

  const choose = useCallback((index: number) => {
    const option = options[index];
    if (option) {
      onChange(option.value);
    }
    setIsOpen(false);
  }, [options, onChange]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    // Ahead of the view's own key handling (capture phase), like the filter
    // menu in the search bar.
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown":
          setHighlightedIndex(i => Math.min(i + 1, options.length - 1));
          break;
        case "ArrowUp":
          setHighlightedIndex(i => Math.max(i - 1, 0));
          break;
        case "Enter":
          choose(highlightedIndex);
          break;
        case "Escape":
          close();
          break;
        default:
          return;
      }
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", handleKeyDown, true);
    // The menu is placed off the button's position, which scrolling or
    // resizing would change.
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [isOpen, highlightedIndex, options.length, choose, close]);

  const menuPosition = (rect: DOMRect) => {
    const height = options.length * ROW_HEIGHT + MENU_PADDING;
    const fitsBelow = rect.bottom + MENU_GAP + height <= window.innerHeight - 4;
    return {
      top: fitsBelow ? rect.bottom + MENU_GAP : rect.top - MENU_GAP - height,
      left: Math.max(4, rect.right - MENU_WIDTH),
    };
  };

  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <span className="text-[13px] text-text-primary">{label}</span>
      <button
        ref={buttonRef}
        type="button"
        className="no-drag flex items-center gap-1 text-[13px] text-text-secondary hover:text-text-primary transition-colors"
        onClick={() => (isOpen ? close() : open())}
      >
        {current?.label ?? value}
        <ChevronDown className="w-3.5 h-3.5" />
      </button>

      {isOpen && anchorRect && createPortal(
        <>
          <div
            className="fixed inset-0 z-[90]"
            onMouseDown={e => e.preventDefault()}
            onClick={close}
          />
          <div
            className="fixed z-[100] bg-bg-secondary/95 backdrop-blur-xl border border-border rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.3)] p-1.5 animate-scale-in origin-top-right"
            style={{ ...menuPosition(anchorRect), width: MENU_WIDTH }}
            onMouseDown={e => e.preventDefault()}
          >
            {options.map((option, index) => (
              <button
                key={option.value}
                type="button"
                className={`w-full flex items-center justify-between px-2.5 py-2 text-[13px] font-medium rounded-lg transition-colors ${
                  index === highlightedIndex ? "bg-bg-hover text-text-primary" : "text-text-secondary"
                }`}
                onClick={() => choose(index)}
                onMouseEnter={() => setHighlightedIndex(index)}
              >
                {option.label}
                {option.value === value && <Check size={14} className="text-accent" />}
              </button>
            ))}
          </div>
        </>,
        document.body,
      )}
    </div>
  );
}
