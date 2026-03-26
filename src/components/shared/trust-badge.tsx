import { trustTier, trustColor } from "@/lib/types";
import { clsx } from "clsx";

interface TrustBadgeProps {
  score: number;
  size?: "sm" | "md";
}

export function TrustBadge({ score, size = "md" }: TrustBadgeProps) {
  const tier = trustTier(score);
  const color = trustColor(score);
  return (
    <span className={clsx("font-mono font-semibold", color, size === "sm" ? "text-xs" : "text-sm")}>
      {score} {tier === "Safe" ? "" : `(${tier.replace("Risk", " Risk")})`}
    </span>
  );
}
