// Grok Build mascot — LobeHub Icons `grok` mark
// https://unpkg.com/@lobehub/icons-static-svg@latest/icons/grok.svg
// Brand color on icons.lobehub.com is #000; we paint with --mascot-icon-color
// so the silhouette stays readable on light and dark cards.
// Motion lives on the .mascot-grok wrapper in mascot.css.

interface MascotSvgProps {
  size: number;
}

const GROK_MARK_D =
  "M9.27 15.29l7.978-5.897c.391-.29.95-.177 1.137.272.98 2.369.542 5.215-1.41 7.169-1.951 1.954-4.667 2.382-7.149 1.406l-2.711 1.257c3.889 2.661 8.611 2.003 11.562-.953 2.341-2.344 3.066-5.539 2.388-8.42l.006.007c-.983-4.232.242-5.924 2.75-9.383.06-.082.12-.164.179-.248l-3.301 3.305v-.01L9.267 15.292M7.623 16.723c-2.792-2.67-2.31-6.801.071-9.184 1.761-1.763 4.647-2.483 7.166-1.425l2.705-1.25a7.808 7.808 0 00-1.829-1A8.975 8.975 0 005.984 5.83c-2.533 2.536-3.33 6.436-1.962 9.764 1.022 2.487-.653 4.246-2.34 6.022-.599.63-1.199 1.259-1.682 1.925l7.62-6.815";

export function GrokMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      fill="currentColor"
      fillRule="evenodd"
      className="grok-svg"
      style={{ color: "var(--mascot-icon-color)", overflow: "visible" }}
      aria-hidden="true"
    >
      {/* The trajectory, drawn under the mark so it reads as passing behind
          the star. Its dashes flow along it while a planet rides it. */}
      <ellipse className="grok-orbit-path" cx="12" cy="12" rx="16.5" ry="4" />
      <path className="grok-mark" d={GROK_MARK_D} />
      {/* The planet: a four-point flare inside a halo that falls off to
          nothing. The concave sides are what make it read as a lens flare
          rather than a diamond — the curve handles sit close to the centre,
          so the four arms taper to points instead of meeting in straight
          edges. Both parts are drawn at the glyph centre and moved by the
          group's transform, so the orbit's keyframes read as coordinates on
          an ellipse. Transparent at rest, so the icon is unchanged everywhere
          it sits still — tables, the marketplace, the kit drawer.
          The gradient carries the tint class too: `currentColor` in a stop
          resolves against the stop's own inherited color in some engines and
          against the referencing element in others, so both are set. */}
      <defs>
        <radialGradient id="grok-glow" className="grok-orbiter-tint">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.5" />
          <stop offset="45%" stopColor="currentColor" stopOpacity="0.2" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </radialGradient>
      </defs>
      <g className="grok-orbiter grok-orbiter-tint">
        <circle cx="12" cy="12" r="3.9" fill="url(#grok-glow)" />
        <path d="M12 8.85C12 11.307 12.693 12 15.15 12C12.693 12 12 12.693 12 15.15C12 12.693 11.307 12 8.85 12C11.307 12 12 11.307 12 8.85Z" />
      </g>
    </svg>
  );
}
