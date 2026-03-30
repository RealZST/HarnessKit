import { trustTier, trustColor } from "@/lib/types";
import { clsx } from "clsx";

interface TrustBadgeProps {
  score: number;
  size?: "sm" | "md";
}

const tierTitle: Record<string, string> = {
  Safe: "Score 80+: No security concerns found",
  LowRisk: "Score 60-79: Minor issues, generally safe",
  NeedsReview: "Score below 60: Review recommended",
};

const tierLabel: Record<string, string> = {
  Safe: "Safe",
  LowRisk: "Low Risk",
  NeedsReview: "Needs Review",
};

export function TrustBadge({ score, size = "md" }: TrustBadgeProps) {
  const tier = trustTier(score);
  const color = trustColor(score);
  return (
    <span title={`${tierLabel[tier]} — ${tierTitle[tier]}`} className={clsx("font-mono font-semibold tabular-nums", color, size === "sm" ? "text-xs" : "text-sm")}>
      {score}
    </span>
  );
}
