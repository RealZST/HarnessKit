// omp (Oh My Pi) mascot — the π symbol from omp.sh/favicon.svg, with a
// pink→purple→cyan gradient on a dark rounded tile.
// Hover: gentle float + hue cycle on the gradient.
// Click: glow burst + scale pulse.
// Source path: https://omp.sh/favicon.svg (viewBox 0 0 64 64)

interface MascotSvgProps {
  size: number;
}

export function OmpMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      <defs>
        <linearGradient id="omp-grad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ed4abf" />
          <stop offset=".5" stopColor="#9b4dff" />
          <stop offset="1" stopColor="#5ad8e6" />
        </linearGradient>
      </defs>
      {/* Dark rounded background tile */}
      <rect className="omp-bg" width="64" height="64" rx="12" fill="#0f0a14" />
      {/* π symbol */}
      <path
        className="omp-pi"
        fill="url(#omp-grad)"
        d="M14 16h36v8H40v32h-8V24h-6v22h-8V24h-4z"
      />
    </svg>
  );
}
