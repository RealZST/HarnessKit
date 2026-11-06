export interface CursorState { x: number; y: number; visible: boolean; clicking: boolean }

export const HIDDEN_CURSOR: CursorState = { x: 0, y: 0, visible: false, clicking: false };

/**
 * Cursor position over `target`, relative to the mock window's padding box (the cursor's containing
 * block). `fx`/`fy` pick the spot inside the target as fractions of its size. Measured at the moment
 * the cursor moves, so it lands on the same element at every viewport width.
 */
export function cursorPointAt(win: HTMLElement | null, target: HTMLElement | null, fx = 0.5, fy = 0.5): Pick<CursorState, "x" | "y"> {
  if (!win || !target) return { x: 0, y: 0 };
  const w = win.getBoundingClientRect();
  const t = target.getBoundingClientRect();
  return {
    x: Math.round(t.left - w.left - win.clientLeft + t.width * fx),
    y: Math.round(t.top - w.top - win.clientTop + t.height * fy),
  };
}

// The macOS arrow, as product demos show it (screen recordings made with Screen Studio, Attio's and Liveblocks' pages):
// black with a white edge and a soft shadow, the same in both themes, a little over the system's size so it reads at
// the mock's scale. Its tip, drawn 2px in from the svg's corner, sits on the target point (mocks.css pulls the svg
// back). A click presses it a little and sends a ring out from the tip; on phones and tablets (mocks.css) the arrow
// gives way to a finger's tap.
export default function MockCursor({ x, y, visible, clicking }: CursorState) {
  return (
    <div className="mockcursor" data-clicking={clicking || undefined} style={{ translate: `${x}px ${y}px`, scale: clicking ? "0.88" : "1", opacity: visible ? 1 : 0 }} aria-hidden="true">
      <svg className="mockcursor-arrow" width="22" height="30" viewBox="0 0 22 30" fill="none">
        <path d="M2 2v21.5l5.2-5 3.6 8.4 3.6-1.5-3.6-8.3H18L2 2Z" fill="var(--cursor-fill)" stroke="var(--cursor-edge)" strokeWidth="1.6" strokeLinejoin="round" />
      </svg>
      <span className="mocktap" />
    </div>
  );
}
