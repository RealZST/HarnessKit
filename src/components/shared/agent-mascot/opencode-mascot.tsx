interface MascotSvgProps {
  size: number;
}

export function OpencodeMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      {/* Static icon */}
      <g className="opencode-icon">
        <path
          className="opencode-outline"
          d="M4 2h16v20H4z"
          fill="none"
          stroke="var(--mascot-icon-color)"
          strokeWidth="2.2"
          strokeLinejoin="round"
        />
        <rect
          className="opencode-core"
          x="8"
          y="6"
          width="8"
          height="12"
          fill="var(--mascot-icon-color)"
        />
      </g>

      {/* Hover: snake traces the outline perimeter */}
      <path
        className="opencode-snake"
        d="M4 2h16v20H4z"
        fill="none"
        stroke="var(--mascot-icon-color)"
        strokeWidth="2.2"
        strokeLinejoin="round"
        strokeLinecap="round"
      />

      {/* Click: mosaic pixel tiles */}
      <g className="opencode-mosaic">
        {/* Border strips approximating the outline stroke */}
        <rect className="oc-tile" x="3" y="0.9" width="18" height="2.2" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="18.9" y="0.9" width="2.2" height="21.2" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="3" y="20.9" width="18" height="2.2" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="2.9" y="0.9" width="2.2" height="21.2" fill="var(--mascot-icon-color)" />
        {/* Inner rect split into 2×2 quadrants */}
        <rect className="oc-tile" x="8"  y="6"  width="4" height="6" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="12" y="6"  width="4" height="6" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="8"  y="12" width="4" height="6" fill="var(--mascot-icon-color)" />
        <rect className="oc-tile" x="12" y="12" width="4" height="6" fill="var(--mascot-icon-color)" />
      </g>
    </svg>
  );
}