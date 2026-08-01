import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Detects when the Meta (Command) key is held down.
 * Returns true when Meta has been held for at least `delay` ms.
 * Immediately resets when Meta is released.
 */
export function useCommandHold(delay = 300): boolean {
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
      // Only trigger on bare Meta key (no other keys pressed simultaneously)
      if (e.key === "Meta" && !timerRef.current && !isHolding) {
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
      if (e.key !== "Meta" && !isHolding) {
        clear();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === "Meta") {
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
