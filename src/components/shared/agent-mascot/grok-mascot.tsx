// Grok Build mascot — a black rounded tile with a white four-point spark,
// drawn from xAI's black/white brand (https://x.ai/build). Motion lives on
// the .mascot-grok wrapper in mascot.css.

interface MascotSvgProps {
  size: number;
}

export function GrokMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 64 64"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      <rect className="grok-tile" width="64" height="64" rx="14" fill="#111111" />
      <path
        className="grok-spark"
        fill="#f5f5f0"
        d="M32 10c1.2 8.4 5.6 12.8 14 14-8.4 1.2-12.8 5.6-14 14-1.2-8.4-5.6-12.8-14-14 8.4-1.2 12.8-5.6 14-14z"
      />
    </svg>
  );
}
