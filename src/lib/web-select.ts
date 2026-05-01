/** Normalize `<select>` chrome across web and desktop webviews.
 *
 * Native macOS Aqua chrome (Tauri's WKWebView on older macOS, e.g.
 * 15.x Sequoia) renders select controls as heavy Aqua-style buttons that
 * clash with the rest of the flat UI. Apple's newer macOS releases ship
 * a flatter native chrome, but we can't rely on that across user OS
 * versions. Stripping native chrome with `appearance: none` and applying
 * a matching custom arrow gives a single consistent rendering everywhere.
 */
export const webSelectStyle: React.CSSProperties = {
  WebkitAppearance: "none",
  appearance: "none",
  backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='12' viewBox='0 0 8 12'%3E%3Cpath d='M4 1L7 4.5H1Z' fill='%23888'/%3E%3Cpath d='M4 11L1 7.5H7Z' fill='%23888'/%3E%3C/svg%3E")`,
  backgroundRepeat: "no-repeat",
  backgroundPosition: "right 8px center",
  paddingRight: "24px",
};

/** Use the normalized layout (`rounded-[6px] h-[26px]`) instead of the
 *  legacy desktop-only (`rounded-lg py-1.5`). Now true everywhere since
 *  desktop also normalizes — kept as a constant for callers that gate
 *  className choice on it; can be inlined later. */
export const isWeb = true;
