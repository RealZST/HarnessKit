// OpenClaw mascot — the upstream brand mark, a round red lobster with two
// claws, two antennae and teal-lit eyes (openclaw/openclaw ui/public/favicon.svg,
// MIT; this is the 24-unit cut as shipped in lobe-icons). The shell and claws
// keep the brand gradient, which reads on both light and dark cards. The two
// antennae are split into separate paths so each can swing on its own root.
// Motion lives on the .mascot-openclaw wrapper in mascot.css: hover hops in
// place with a squash on touchdown, antennae wagging and eyes blinking; click
// is a pat on the head — the lobster sinks contentedly and rocks on its feet,
// its eyes close into smiling arcs, its cheeks blush and the antennae go limp.

import { useId } from "react";

interface MascotSvgProps {
  size: number;
}

export function OpenClawMascot({ size }: MascotSvgProps) {
  // Gradient ids must be unique per instance: the extensions table renders
  // one mascot per row, and a duplicate id resolved to a hidden copy paints
  // nothing.
  const id = useId();
  return (
    <svg
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      <defs>
        <linearGradient
          id={`${id}shell`}
          gradientUnits="userSpaceOnUse"
          x1="-.659"
          y1=".458"
          x2="27.023"
          y2="22.855"
        >
          <stop stopColor="#FF4D4D" />
          <stop offset="1" stopColor="#991B1B" />
        </linearGradient>
        <linearGradient
          id={`${id}claw-l`}
          gradientUnits="userSpaceOnUse"
          x1="0"
          y1="9.672"
          x2="4.311"
          y2="14.949"
        >
          <stop stopColor="#FF4D4D" />
          <stop offset="1" stopColor="#991B1B" />
        </linearGradient>
        <linearGradient
          id={`${id}claw-r`}
          gradientUnits="userSpaceOnUse"
          x1="19.385"
          y1="9.953"
          x2="24.399"
          y2="14.462"
        >
          <stop stopColor="#FF4D4D" />
          <stop offset="1" stopColor="#991B1B" />
        </linearGradient>
        <radialGradient id={`${id}blush`}>
          <stop stopColor="#FFC2D4" stopOpacity=".95" />
          <stop offset="1" stopColor="#FFC2D4" stopOpacity="0" />
        </radialGradient>
      </defs>
      <g className="oc-lob">
        <path
          className="oc-ant-l"
          fill="#FF4D4D"
          d="M5.507 1.875c.476-.285 1.036-.233 1.615.037.577.27 1.223.774 1.937 1.488a.316.316 0 01-.447.447c-.693-.693-1.279-1.138-1.757-1.361-.475-.222-.795-.205-1.022-.069a.317.317 0 01-.326-.542z"
        />
        <path
          className="oc-ant-r"
          fill="#FF4D4D"
          d="M16.877 1.913c.58-.27 1.14-.323 1.616-.038a.317.317 0 01-.326.542c-.227-.136-.547-.153-1.022.069-.478.223-1.064.668-1.756 1.361a.316.316 0 11-.448-.447c.714-.714 1.36-1.218 1.936-1.487z"
        />
        <path
          fill={`url(#${id}shell)`}
          d="M12 2.568c-6.33 0-9.495 5.275-9.495 9.495 0 4.22 3.165 8.44 6.33 9.494v2.11h2.11v-2.11s1.055.422 2.11 0v2.11h2.11v-2.11c3.165-1.055 6.33-5.274 6.33-9.494S18.33 2.568 12 2.568z"
        />
        <path
          fill={`url(#${id}claw-l)`}
          d="M3.56 9.953C.396 8.898-.66 11.008.396 13.118c1.055 2.11 3.164 1.055 4.22-1.055.632-1.477 0-2.11-1.056-2.11z"
        />
        <path
          fill={`url(#${id}claw-r)`}
          d="M20.44 9.953c3.164-1.055 4.22 1.055 3.164 3.165-1.055 2.11-3.164 1.055-4.22-1.055-.632-1.477 0-2.11 1.056-2.11z"
        />
        <g className="oc-eyes">
          <path
            fill="#050810"
            d="M8.835 9.109a1.266 1.266 0 100-2.532 1.266 1.266 0 000 2.532zM15.165 9.109a1.266 1.266 0 100-2.532 1.266 1.266 0 000 2.532z"
          />
          <path
            fill="#00E5CC"
            d="M9.046 8.16a.527.527 0 100-1.056.527.527 0 000 1.055zM15.376 8.16a.527.527 0 100-1.055.527.527 0 000 1.054z"
          />
        </g>
        <path
          className="oc-smile"
          d="M7.6 8.5Q8.835 6.8 10.07 8.5M13.93 8.5Q15.165 6.8 16.4 8.5"
        />
        <g className="oc-blush" fill={`url(#${id}blush)`}>
          <ellipse cx="7.2" cy="10.7" rx="1.9" ry="1.1" />
          <ellipse cx="16.8" cy="10.7" rx="1.9" ry="1.1" />
        </g>
      </g>
    </svg>
  );
}
