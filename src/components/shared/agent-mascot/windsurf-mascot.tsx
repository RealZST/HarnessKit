interface MascotSvgProps {
  size: number;
}

// Windsurf logo path: closed fill outline of the double-M ribbon shape.
const WINDSURF_LOGO_D =
  "M507.28 108.142623H502.4C476.721 108.10263 455.882 128.899 455.882 154.5745V258.416C455.882 279.153 438.743 295.95 418.344 295.95C406.224 295.95 394.125 289.851 386.945 279.613L280.889 128.1391C272.089 115.56133 257.77 108.0626373 242.271 108.0626373C218.091 108.0626373 196.332 128.6191 196.332 153.9946V258.436C196.332 279.173 179.333 295.97 158.794 295.97C146.634 295.97 134.555 289.871 127.375 279.633L8.69966 110.12228C6.01976 106.28295 0 108.182617 0 112.8618V203.426C0 208.005 1.39995 212.444 4.01984 216.204L120.815 382.995C127.715 392.853 137.895 400.172 149.634 402.831C179.013 409.51 206.052 386.894 206.052 358.079V253.697C206.052 232.961 222.851 216.164 243.59 216.164H243.65C256.15 216.164 267.87 222.263 275.049 232.501L381.125 383.955C389.945 396.552 403.524 404.031 419.724 404.031C444.443 404.031 465.622 383.455 465.622 358.099V253.677C465.622 232.941 482.421 216.144 503.16 216.144H507.3C509.9 216.144 512 214.044 512 211.445V112.8418C512 110.24226 509.9 108.142623 507.3 108.142623H507.28Z";

// Mask path traversing the W centerline as 4 quadratic Bezier segments.
// Using Q curves with control points slightly offset from the straight-line
// midpoints (toward the W interior) keeps the W shape recognizable while
// removing the hard-corner segment-to-segment transitions that make a pure
// L-command path feel jittery as the dash sweeps through joins.
const WINDSURF_MASK_D =
  "M 0 113 Q 95 250 150 403 Q 210 245 242 108 Q 315 248 420 404 Q 450 248 512 113";

export function WindsurfMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 512 512"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      <defs>
        <mask id="windsurf-write-mask" maskUnits="userSpaceOnUse">
          <rect width="512" height="512" fill="black" />
          <path
            className="windsurf-mask-stroke"
            d={WINDSURF_MASK_D}
            stroke="white"
            strokeWidth="220"
            strokeLinecap="round"
            strokeLinejoin="round"
            fill="none"
          />
        </mask>
      </defs>
      <g className="windsurf-logo" mask="url(#windsurf-write-mask)">
        <path d={WINDSURF_LOGO_D} fill="var(--mascot-icon-color)" />
      </g>
    </svg>
  );
}
