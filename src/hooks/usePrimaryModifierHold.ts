import { useCallback, useEffect, useRef, useState } from "react";
import { PRIMARY_MODIFIER_KEY } from "../lib/platform";

/**
 * Detects when the shortcut modifier (⌘ on macOS, Ctrl elsewhere) is held down.
 * Returns true when it has been held for at least `delay` ms.
 * Immediately resets when it is released.
 */
export function usePrimaryModifierHold(delay = 300): boolean {
  const [isHolding, setIsHolding] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clear = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Only trigger on the bare modifier (no other keys pressed simultaneously)
      if (e.key === PRIMARY_MODIFIER_KEY && !timerRef.current && !isHolding) {
        // The cleanup below calls clear(), which clearTimeout()s this handle —
        // the rule just can't follow it through the indirection.
        // eslint-disable-next-line react/web-api-no-leaked-timeout
        timerRef.current = setTimeout(() => {
          setIsHolding(true);
          timerRef.current = null;
        }, delay);
      }

      // If any other key is pressed while waiting, cancel the hold detection
      // But if we're already showing badges, don't cancel — let number keys work
      if (e.key !== PRIMARY_MODIFIER_KEY && !isHolding) {
        clear();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === PRIMARY_MODIFIER_KEY) {
        clear();
        setIsHolding(false);
      }
    };

    // Also reset when window loses focus
    const handleBlur = () => {
      clear();
      setIsHolding(false);
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);

    return () => {
      clear();
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, [delay, isHolding, clear]);

  return isHolding;
}
