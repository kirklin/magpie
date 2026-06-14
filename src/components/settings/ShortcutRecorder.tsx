import { X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface ShortcutRecorderProps {
  label: string;
  description?: string;
  value: string;
  onChange: (shortcut: string) => Promise<void>;
}

/** Map e.code to a Tauri-compatible key name */
const CODE_TO_KEY: Record<string, string> = {
  KeyA: "A",
  KeyB: "B",
  KeyC: "C",
  KeyD: "D",
  KeyE: "E",
  KeyF: "F",
  KeyG: "G",
  KeyH: "H",
  KeyI: "I",
  KeyJ: "J",
  KeyK: "K",
  KeyL: "L",
  KeyM: "M",
  KeyN: "N",
  KeyO: "O",
  KeyP: "P",
  KeyQ: "Q",
  KeyR: "R",
  KeyS: "S",
  KeyT: "T",
  KeyU: "U",
  KeyV: "V",
  KeyW: "W",
  KeyX: "X",
  KeyY: "Y",
  KeyZ: "Z",
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
  Space: "Space",
  Backspace: "Backspace",
  Delete: "Delete",
  Enter: "Enter",
  Tab: "Tab",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

const KEY_DISPLAY: Record<string, string> = {
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  Space: "Space",
  Backspace: "⌫",
  Delete: "⌦",
  Enter: "↩",
  Tab: "⇥",
  Escape: "⎋",
};

function formatShortcutDisplay(shortcut: string): string[] {
  const parts = shortcut.split("+");
  const symbols: string[] = [];
  for (const part of parts) {
    switch (part) {
      case "CmdOrCtrl": case "Cmd": symbols.push("⌘"); break;
      case "Ctrl": symbols.push("⌃"); break;
      case "Shift": symbols.push("⇧"); break;
      case "Alt": case "Option": symbols.push("⌥"); break;
      default: symbols.push(KEY_DISPLAY[part] ?? part);
    }
  }
  return symbols;
}

function keyEventToShortcut(e: KeyboardEvent): string | null {
  if (!e.metaKey && !e.ctrlKey && !e.altKey) {
    return null;
  }
  const modifierCodes = new Set([
    "MetaLeft",
    "MetaRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "ShiftLeft",
    "ShiftRight",
    "CapsLock",
  ]);
  if (modifierCodes.has(e.code)) {
    return null;
  }
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) {
    parts.push("CmdOrCtrl");
  }
  if (e.altKey) {
    parts.push("Alt");
  }
  if (e.shiftKey) {
    parts.push("Shift");
  }
  const key = CODE_TO_KEY[e.code];
  if (!key) {
    return null;
  }
  parts.push(key);
  return parts.join("+");
}

function KeyCap({ children }: { children: string }) {
  return (
    <kbd className="inline-flex items-center justify-center min-w-[22px] h-[22px] px-1.5 rounded-[4px] bg-bg-hover text-[11px] font-sans font-medium leading-none text-text-primary shadow-[0_1px_0_rgba(0,0,0,0.15)] border border-border">
      {children}
    </kbd>
  );
}

/** Popover rendered via portal to avoid parent overflow clipping */
function RecordingPopover({
  anchorRect,
  onClose,
}: {
  anchorRect: DOMRect;
  onClose: () => void;
}) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  useLayoutEffect(() => {
    // Position above the anchor, right-aligned
    const popoverW = 220;
    const popoverH = 80;
    const gap = 8;

    let top = anchorRect.top - popoverH - gap;
    let left = anchorRect.right - popoverW;

    // Clamp to viewport
    if (top < 4) {
      top = anchorRect.bottom + gap;
    }
    if (left < 4) {
      left = 4;
    }

    setPos({ top, left });
  }, [anchorRect]);

  return createPortal(
    <div ref={popoverRef} className="fixed z-[100]" style={{ top: pos.top, left: pos.left }}>
      <div className="relative bg-bg-secondary border border-border rounded-xl shadow-[0_12px_40px_rgba(0,0,0,0.25)] w-[220px] overflow-hidden">
        {/* Close button */}
        <button
          type="button"
          className="absolute top-2 right-2 w-5 h-5 flex items-center justify-center rounded text-text-tertiary hover:text-text-primary hover:bg-bg-hover transition-colors"
          onClick={onClose}
        >
          <X size={11} />
        </button>

        {/* Example hint */}
        <div className="flex items-center justify-center gap-1.5 px-4 pt-3 pb-1.5">
          <span className="text-[11px] text-text-tertiary mr-1">e.g.</span>
          <KeyCap>⇧</KeyCap>
          <KeyCap>⌘</KeyCap>
          <KeyCap>Space</KeyCap>
        </div>

        {/* Recording label */}
        <div className="text-[12px] text-text-secondary text-center pb-3">
          Recording...
        </div>
      </div>

      {/* Arrow pointing down to anchor */}
      <div
        className="absolute w-0 h-0"
        style={{
          right: Math.max(16, anchorRect.width / 2),
          bottom: -6,
          borderLeft: "6px solid transparent",
          borderRight: "6px solid transparent",
          borderTop: "6px solid var(--color-bg-secondary)",
        }}
      />
    </div>,
    document.body,
  );
}

export function ShortcutRecorder({ label, description, value, onChange }: ShortcutRecorderProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [isError, setIsError] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const openRecording = () => {
    if (buttonRef.current) {
      setAnchorRect(buttonRef.current.getBoundingClientRect());
    }
    setIsRecording(true);
  };

  const closeRecording = () => {
    setIsRecording(false);
  };

  const handleKeyDown = useCallback(async (e: KeyboardEvent) => {
    if (!isRecording) {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      closeRecording(); return;
    }
    const shortcut = keyEventToShortcut(e);
    if (!shortcut) {
      return;
    }
    closeRecording();
    try {
      await onChange(shortcut);
      setIsError(false);
      setErrorMsg("");
    } catch (err) {
      setIsError(true);
      setErrorMsg(String(err));
      setTimeout(() => {
        setIsError(false); setErrorMsg("");
      }, 3000);
    }
  }, [isRecording, onChange]);

  useEffect(() => {
    if (isRecording) {
      window.addEventListener("keydown", handleKeyDown, true);
      return () => window.removeEventListener("keydown", handleKeyDown, true);
    }
  }, [isRecording, handleKeyDown]);

  // Click outside to cancel
  useEffect(() => {
    if (!isRecording) {
      return;
    }
    const handleClick = (e: MouseEvent) => {
      // Close if clicking outside the button (popover is in portal)
      if (buttonRef.current && !buttonRef.current.contains(e.target as Node)) {
        closeRecording();
      }
    };
    // Delay to avoid catching the opening click
    const timer = setTimeout(() => {
      window.addEventListener("mousedown", handleClick);
    }, 0);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("mousedown", handleClick);
    };
  }, [isRecording]);

  const displayParts = formatShortcutDisplay(value);

  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <div className="flex flex-col">
        <span className="text-[13px] text-text-primary">{label}</span>
        {description && <span className="text-[11px] text-text-tertiary mt-0.5">{description}</span>}
        {isError && errorMsg && (
          <span className="text-[11px] text-red-400 mt-0.5">{errorMsg}</span>
        )}
      </div>

      <button
        ref={buttonRef}
        type="button"
        className={`no-drag flex items-center gap-[3px] px-2 py-1 rounded-md transition-colors duration-150 ${
          isRecording
            ? "ring-1 ring-accent/60 bg-bg-active"
            : "bg-bg-hover hover:bg-bg-active"
        }`}
        onClick={openRecording}
      >
        {displayParts.map((sym, i) => (
          <KeyCap key={i}>{sym}</KeyCap>
        ))}
      </button>

      {/* Portal-rendered popover */}
      {isRecording && anchorRect && (
        <RecordingPopover anchorRect={anchorRect} onClose={closeRecording} />
      )}
    </div>
  );
}
