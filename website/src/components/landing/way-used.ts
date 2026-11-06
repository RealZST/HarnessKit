// The ways in (InstallOptions.astro, InstallCommand.tsx) each say what to do next after a click or a copy, and keep
// saying it. Only the latest one speaks: a way announces its use, and every other way clears its notice.
const WAY_USED = "hk:way-used";

/** Tells the other ways that `way` was just used. */
export function announceWayUsed(way: Element) {
  document.dispatchEvent(new CustomEvent(WAY_USED, { detail: way }));
}

/** Calls `clear` when another way is used, or the page is left. Returns the unsubscribe. */
export function onWayLeft(way: Element, clear: () => void): () => void {
  const other = (e: Event) => {
    if ((e as CustomEvent<Element>).detail !== way) clear();
  };
  document.addEventListener(WAY_USED, other);
  window.addEventListener("pagehide", clear);
  return () => {
    document.removeEventListener(WAY_USED, other);
    window.removeEventListener("pagehide", clear);
  };
}
