import { useEffect } from "react";

/** Run `onClose` when the user presses Escape. */
export function useEscape(onClose: () => void, enabled: boolean = true) {
  useEffect(() => {
    if (!enabled) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, enabled]);
}
