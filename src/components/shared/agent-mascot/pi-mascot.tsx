// Pi mascot — the upstream brand mark (pi.dev/logo-auto.svg): a pixel π
// built from three blocks on a 4×4 grid, a coral bar, a blue left leg and
// a yellow right leg. Drawn at 10 units per cell without a tile, which also
// keeps it apart from omp's gradient π on a dark tile. Each block has a
// darker twin behind it (.pi-depth) that sits exactly under it at rest and
// slides out on hover as the block's side face, so the flat mark turns into
// blocks with thickness. Blocks paint a same-colour stroke 0.3 units past
// their edges to cover the anti-aliased seam where two of them meet.
// Motion lives on the .mascot-pi wrapper in mascot.css: hover lifts the
// blocks one by one; click slides them together into a 3×3 square with one
// cell left over, holds, then the square puffs away in a burst of smoke
// and the π fades back in.

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

// Smoke puffs for the vanish, in viewBox units around the square's centre.
// Each drifts along its own vector so the burst reads as a puff rather
// than a ring; fixed so every click looks the same.
const PUFFS = [
  { px: -16, py: -13, delay: 0 },
  { px: 15, py: -15, delay: 0.04 },
  { px: 18, py: 6, delay: 0.02 },
  { px: -14, py: 13, delay: 0.06 },
  { px: 0, py: -19, delay: 0.08 },
  { px: 12, py: 17, delay: 0.05 },
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
      {PUFFS.map((p) => (
        <circle
          key={`${p.px},${p.py}`}
          className="pi-puff"
          cx="20"
          cy="18"
          r="4.2"
          style={
            {
              "--px": `${p.px}px`,
              "--py": `${p.py}px`,
              "--delay": `${p.delay}s`,
            } as React.CSSProperties
          }
        />
      ))}
    </svg>
  );
}
