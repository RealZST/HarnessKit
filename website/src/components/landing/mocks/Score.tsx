import type { CSSProperties } from "react";
import { tierTone } from "./mock-data";

// Tier color on a dot, the number in ink: the trust tones are too light for text.
export default function Score({ score }: { score: number }) {
  return (
    <span className="mock-score">
      <span className="mock-dot" style={{ "--tone": tierTone(score) } as CSSProperties} />
      {score}
    </span>
  );
}
