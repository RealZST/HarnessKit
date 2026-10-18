// Pi mascot — the upstream brand mark (pi.dev/logo-auto.svg): a pixel π
// built from three blocks on a 4×4 grid, a coral bar, a blue left leg and
// a yellow right leg. Drawn at 10 units per cell without a tile, which also
// keeps it apart from omp's gradient π on a dark tile. Each block has a
// darker twin behind it (.pi-depth) that sits exactly under it at rest and
// slides out on hover as the block's side face, so the flat mark turns into
// blocks with thickness. Blocks paint a same-colour stroke 0.3 units past
// their edges to cover the anti-aliased seam where two of them meet.
// Motion lives on the .mascot-pi wrapper in mascot.css: hover lifts the
// blocks one by one; click replays the pi.dev hero animation, a game of
// Tetris that leaves the π behind. The three extra shapes exist only for
// that replay: the turquoise bar and the yellow leg's foot fill the row
// under the π that gets cleared, and the white strip is the row's flash.

interface MascotSvgProps {
  size: number;
}

const BLOCKS = [
  { cls: "pi-bar", d: "M0 0h30v20H20V10H0z", fill: "#F09082", side: "#B5665B" },
  {
    cls: "pi-left",
    d: "M0 10h10v10h10v10H10v10H0z",
    fill: "#4D9ABF",
    side: "#33708C",
  },
  { cls: "pi-right", d: "M30 20h10v20H30z", fill: "#F1BE58", side: "#B58A3A" },
];

// The row that gets cleared, one cell wider than the π on each side: the
// four-cell turquoise bar (pi.dev's fourth brand colour) and the two cells
// the yellow leg loses. Drawn where they land, hidden until a click.
const CLEARED_ROW = [
  { cls: "pi-base", d: "M-10 30h40v10H-10z", fill: "#83CCD2" },
  { cls: "pi-foot", d: "M30 30h20v10H30z", fill: "#F1BE58" },
];

export function PiMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 40 40"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      {BLOCKS.map((b) => (
        <path
          key={b.cls}
          className={`pi-depth ${b.cls}`}
          d={b.d}
          fill={b.side}
          stroke={b.side}
        />
      ))}
      {BLOCKS.map((b) => (
        <path
          key={b.cls}
          className={`pi-piece ${b.cls}`}
          d={b.d}
          fill={b.fill}
          stroke={b.fill}
        />
      ))}
      {CLEARED_ROW.map((c) => (
        <path
          key={c.cls}
          className={c.cls}
          d={c.d}
          fill={c.fill}
          stroke={c.fill}
        />
      ))}
      {/* Stroked like the blocks so it covers their seam strokes; a touch
          translucent so on a light card the row reads as washed out
          rather than gone. */}
      <path
        className="pi-flash"
        d="M-10 30h60v10H-10z"
        fill="#fff"
        stroke="#fff"
        fillOpacity={0.88}
        strokeOpacity={0.88}
      />
    </svg>
  );
}
