import { useCallback, useEffect, useState } from "react";

/** Collapsed boolean persisted in localStorage under `storageKey`. Pass `null`
 *  to keep it session-only. Rehydrates when the key changes.
 *
 *  `setCollapsed` is referentially stable (memoized on the key) and safe to use
 *  in effect deps. `toggle` is NOT stable — its identity changes whenever
 *  `collapsed` changes — so use it as an event handler, not in effect deps. */
export function useCollapsibleState(storageKey: string | null): {
  collapsed: boolean;
  setCollapsed: (value: boolean) => void;
  toggle: () => void;
} {
  const [collapsed, setCollapsedState] = useState<boolean>(() =>
    storageKey ? localStorage.getItem(storageKey) === "1" : false,
  );

  useEffect(() => {
    if (!storageKey) return;
    setCollapsedState(localStorage.getItem(storageKey) === "1");
  }, [storageKey]);

  const setCollapsed = useCallback(
    (value: boolean) => {
      setCollapsedState(value);
      if (storageKey) {
        if (value) localStorage.setItem(storageKey, "1");
        else localStorage.removeItem(storageKey);
      }
    },
    [storageKey],
  );

  const toggle = useCallback(
    () => setCollapsed(!collapsed),
    [setCollapsed, collapsed],
  );

  return { collapsed, setCollapsed, toggle };
}
