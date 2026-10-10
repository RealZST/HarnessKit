// PenguinHarness mascot — the upstream brand mark, a penguin gliding up
// and to the right through two wake lines.
// github.com/Prism-Shadow/penguin-harness packages/web/public/penguin-logo.svg
// (Apache-2.0). Upstream paints the body with a navy gradient on a white
// tile; the belly and face are holes in the body path, so the mark is used
// without the tile and the body takes --mascot-icon-color to survive dark
// cards. Beak and feet keep the brand blue, as does the wake. Motion lives
// on the .mascot-penguin wrapper in mascot.css: hover keeps water sliding
// past (the wake drifts back and fades while a second copy fades in behind
// the penguin, which surges forward with each new wake); click stops the
// penguin to shake itself dry, the wake scattering into the droplets and
// returning once it swims on.

interface MascotSvgProps {
  size: number;
}

const WAKE_D =
  "M475 558v3c-1 0-3 1-4 1-16 4-31 9-47 14-1 0-2 1-4 1-76 26-179 67-217 143-7 15-9 29-4 45 2 5 4 10 8 14s4 4 4 6c1 1 3 1 4 2s2 1 3 2c9 6 18 10 29 14 4 1 4 1 8 3v2c-23 4-48-1-67-14-12-9-19-19-22-34-2-28 8-50 27-71 1-2 3-4 5-6s4-4 5-6l3-3h2c1-1 1-3 2-4 20-19 44-35 69-48 1-1 3-2 4-2 14-8 29-15 44-21l9-3c36-15 74-26 112-34 1 0 2 0 3-1 17-3 17-3 24-3m571 110c-7 13-22 25-33 35l-3 3c-7 6-15 12-23 18-1 1-2 2-3 2-71 54-153 93-236 122-1 0-2 1-3 1-31 11-63 20-95 27-1 0-1 0-4 1-65 14-132 17-198 8 0-1-1-1-1-2 4-1 7-2 11-2 141-16 278-54 423-118 1 0 2-1 3-1 26-12 51-24 75-38 1 0 1 0 5-3 14-8 27-16 40-24 1-1 2-1 4-2 11-8 21-16 32-24 4-3 4-3 6-3";

const BODY_D =
  "M532 329c35 18 68 41 97 68l9 6 12 12c3 3 6 6 10 9 3 3 6 5 8 8 36 34 36 34 56 42l-3-3-17-17c1-5 2-8 5-13 0-1 0-1 2-4 1-2 1-4 2-5 1-2 2-3 2-5 13-26 25-51 45-72 0-1 0-1 2-3 24-26 60-37 94-39 20-1 39 1 58 6 19 6 39 8 58 2-1 6-3 10-7 14-3 2-6 4-9 5 1 1 2 1 3 2 16 11 26 24 30 44 3 18 3 18 0 24h3c50-4 50-4 71 10 2 2 2 2 3 7-5 5-10 6-16 8-31 9-53 23-72 49-1 1-2 2-3 4-24 33-41 70-57 108-17 42-39 80-68 114-1 1-1 2-2 2-10 12-22 24-34 34-2 1-4 3-6 5-12 10-24 18-37 26-1 1-3 2-4 3-73 47-165 69-251 56-21-3-38 2-58 8-1 0-1 0-4 1-2 1-3 1-4 2-2 0-3 0-4 1-6 2-8 7-11 13-4 7-8 14-13 20l-3 3c-9 11-19 23-34 26-6 0-7 0-11-4-2-3-1-6-1-10v-3c-1 1-3 2-4 3-7 4-12 6-20 5-4-2-5-3-6-7 0-8 3-13 8-18h-3c-4-1-4-1-6-3-2-4-2-5-1-9 17-29 46-44 77-53-17-3-37-6-53 2-2 2-4 5-6 7-4 6-9 11-14 16l-3 3c-12 12-27 25-45 29-5 0-6 0-10-4-2-2-2-2-2-6 1-5 2-7 4-11-10 3-10 3-13 4-5 1-8 1-13 0-4-4-4-4-5-8 1-7 6-11 11-16-2-1-3-1-5-2l-2-2c1-11 11-18 19-25 18-16 40-27 63-35 5-2 9-4 13-8l4-4c-2 0-3-1-5-1-18-4-34-10-50-21v-3h3c33-3 64-17 86-42 3-3 6-7 8-11 3-3 6-7 10-10 2-3 4-5 6-8 6-7 13-14 19-21 2-1 3-2 4-3 7-7 14-14 22-20 2-2 4-4 6-5 32-27 68-49 104-69l3 3c9 7 20 10 32 12-2-1-3-2-5-3-24-15-41-37-55-62 0-1 0-1-3-5-5-8-9-16-14-25-15-27-32-52-51-76-1-1-2-3-3-4-6-8-13-15-19-23-4-4-8-9-12-14-1-1-2-2-3-4-12-15-12-15-12-23 1-4 1-4 3-6 25-15 74 11 97 22m288 38-3 3c-16 14-26 35-27 56 0 15 1 27 8 40 1 1 1 2 2 3 10 18 25 29 44 38 2 0 3 1 5 1v2l-3 1c-43 10-81 41-108 76-2 2-4 5-6 7-4 5-8 11-12 16-5 7-11 14-17 21-2 3-5 7-8 10-47 58-107 90-180 104-30 6-56 19-81 38-2 2-4 3-6 4-2 2-3 3-5 4 3 3 9 4 13 6 7 2 14 4 21 7 27 10 54 17 83 20h4c80 7 163-20 228-65 1-1 2-2 4-3 25-17 50-37 69-61 2-2 3-5 5-7 28-32 46-71 62-109 17-40 36-78 63-112-3-2-6-3-10-4-15-4-26-12-37-23 1-2 2-3 3-5 7-10 9-21 7-33-4-19-12-32-29-43-29-16-64-13-89 8";

const BEAK_D =
  "M985 405h3c0 2 1 4 1 6h4c49-5 49-5 67 7 4 4 4 4 6 7-1 2-1 3-2 5-1 0-2 1-4 1-3 1-6 2-10 3l-9 3c-19 6-35 15-49 30l-3 3-5 5-6 6c-3-1-5-3-8-4 1-5 2-7 6-11l-12-3c0-1-1-3-1-4 1-1 1-2 2-3 3-5 5-9 6-14 1-2 1-3 2-5 2-6 2-6 2-8l3-3c1-3 1-5 2-8 1-5 2-8 5-13";

const FOOT_L_D =
  "M308 782c1 0 1 1 2 1v4c1 1 3 1 4 2v2c2 0 3-1 5-1l2 2c8 8 15 11 26 11 3 1 7 3 10 4h5l3 1c-2 8-8 14-14 20-2 3-3 5-5 7-13 14-31 31-51 32-5 0-5 0-9-3-2-6 0-11 3-17q-4.5 0-9 3c-5 2-10 2-15 1-3-1-3-1-6-4-1-5 0-8 3-12 2-3 4-5 7-8l-6-3c0-2-1-4-1-6 10-12 21-23 35-31h3v-2c3-1 5-2 8-3";

const FOOT_R_D =
  "M393 824c1 1 2 1 3 2h4c6 1 7 3 11 6 3 1 7 2 10 3 1 1 2 1 4 2 3 1 3 1 8 2 6 1 7 2 10 7l-1 3h-2c-1 2-1 3-2 5-9 19-25 45-45 53-6 2-10 4-16 2-4-4-4-5-4-10 0-2 1-4 1-6-1 0-2 1-3 1-1 1-2 2-3 2-1 1-3 2-4 2-5 3-9 3-15 3-3-2-5-3-6-7 0-7 3-13 8-18-1 0-3-1-4-1-4-1-4-1-6-3-1-5 0-7 2-11 1-2 2-3 3-4s1-2 2-3l8-8c0-1 0-1 3-3 23-22 23-22 34-19";

const EYE_D =
  "M880 387c10 6 19 13 23 25 1 7 1 12-2 18-4 5-8 8-15 8-8 0-11-2-16-7-4-9-3-17 0-25 2-4 2-4 6-8h2c-4-6-10-8-17-10h-15c2-4 2-4 5-6 10-3 20 1 29 5m-3 16c-2 2-2 2-2 6s0 4 4 8c5-3 5-3 6-6 0-3 0-3-3-8Z";

// Shake-off droplets, in viewBox units. Each carries its own fling vector
// and timing so the burst reads as scattered water rather than a ring of
// dots; the layout is fixed so every click looks the same.
const DROPS = [
  {
    cx: 671,
    cy: 651,
    rx: 63,
    ry: 36,
    rot: 177,
    fill: "#a3c8ff",
    fx: 19,
    fy: 253,
    delay: 0.27,
    dur: 0.81,
  },
  {
    cx: 505,
    cy: 625,
    rx: 39,
    ry: 29,
    rot: 79,
    fill: "#7fb3ff",
    fx: -233,
    fy: 69,
    delay: 0.28,
    dur: 0.95,
  },
  {
    cx: 490,
    cy: 576,
    rx: 68,
    ry: 57,
    rot: 57,
    fill: "#7fb3ff",
    fx: -197,
    fy: 89,
    delay: 0.05,
    dur: 0.98,
  },
  {
    cx: 522,
    cy: 574,
    rx: 63,
    ry: 40,
    rot: 167,
    fill: "#7fb3ff",
    fx: -310,
    fy: -26,
    delay: 0.41,
    dur: 0.91,
  },
  {
    cx: 475,
    cy: 578,
    rx: 50,
    ry: 47,
    rot: 136,
    fill: "#7fb3ff",
    fx: -353,
    fy: -11,
    delay: 0.31,
    dur: 0.75,
  },
  {
    cx: 496,
    cy: 517,
    rx: 73,
    ry: 63,
    rot: 75,
    fill: "#a3c8ff",
    fx: -333,
    fy: -93,
    delay: 0.2,
    dur: 0.84,
  },
  {
    cx: 461,
    cy: 494,
    rx: 46,
    ry: 32,
    rot: 154,
    fill: "#7fb3ff",
    fx: -325,
    fy: -73,
    delay: 0.1,
    dur: 0.93,
  },
  {
    cx: 501,
    cy: 489,
    rx: 46,
    ry: 37,
    rot: 3,
    fill: "#a3c8ff",
    fx: -297,
    fy: -206,
    delay: 0.21,
    dur: 1.13,
  },
  {
    cx: 783,
    cy: 500,
    rx: 71,
    ry: 62,
    rot: 45,
    fill: "#7fb3ff",
    fx: 264,
    fy: -177,
    delay: 0.28,
    dur: 0.97,
  },
];

export function PenguinMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="139 129 954 954"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
      aria-hidden="true"
    >
      <path className="penguin-wake" d={WAKE_D} fill="#015dfc" />
      {/* Second wake for the hover loop, half a cycle behind; invisible at rest. */}
      <path
        className="penguin-wake penguin-wake-echo"
        d={WAKE_D}
        fill="#015dfc"
      />
      <g className="penguin-body">
        <path d={BODY_D} fill="var(--mascot-icon-color)" />
        <path d={BEAK_D} fill="#015dfc" />
        <path d={FOOT_L_D} fill="#015dfc" />
        <path d={FOOT_R_D} fill="#015dfc" />
        <path d={EYE_D} fill="var(--mascot-icon-color)" />
      </g>
      {/* Droplets for the click shake; invisible at rest. */}
      {DROPS.map((d) => (
        <ellipse
          key={`${d.cx}-${d.cy}`}
          className="penguin-drop"
          cx={d.cx}
          cy={d.cy}
          rx={d.rx}
          ry={d.ry}
          transform={`rotate(${d.rot} ${d.cx} ${d.cy})`}
          fill={d.fill}
          style={
            {
              "--fx": `${d.fx}px`,
              "--fy": `${d.fy}px`,
              "--delay": `${d.delay}s`,
              "--dur": `${d.dur}s`,
            } as React.CSSProperties
          }
        />
      ))}
    </svg>
  );
}
