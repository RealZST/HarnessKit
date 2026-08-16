// dsh (DeepSeek Harness) mascot — a minimal whale glyph on a dark rounded
// tile, DeepSeek-blue gradient. Follows the omp-mascot structure.

interface MascotSvgProps {
  size: number;
}

export function DshMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      <defs>
        <linearGradient id="dsh-grad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#4d6bfe" />
          <stop offset="1" stopColor="#38bdf8" />
        </linearGradient>
      </defs>
      <rect className="dsh-bg" width="64" height="64" rx="12" fill="#0b1020" />
      {/* whale body + tail */}
      <path
        className="dsh-whale"
        fill="url(#dsh-grad)"
        d="M10 36c2-9 11-15 22-15 9 0 16 4 20 10l6-5c1-1 3 0 2 2l-3 7 3 7c1 2-1 3-2 2l-6-5c-4 6-11 10-20 10-11 0-20-6-22-13z"
      />
      {/* eye */}
      <circle cx="22" cy="34" r="2.4" fill="#0b1020" />
    </svg>
  );
}
