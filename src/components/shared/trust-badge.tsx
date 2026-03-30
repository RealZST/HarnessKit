import { trustTier, trustColor } from "@/lib/types";
import { clsx } from "clsx";

interface TrustBadgeProps {
  score: number;
  size?: "sm" | "md";
}

const tierTitle: Record<string, string> = {
  Safe: "Score 80+: No security concerns found",
  LowRisk: "Score 60-79: Minor issues, generally safe",
  HighRisk: "Score 40-59: Review recommended",
  Critical: "Score below 40: Significant security concerns",
};

const tierLabel: Record<string, string> = {
  Safe: "Safe",
  LowRisk: "Low Risk",
  HighRisk: "Needs Review",
  Critical: "Critical",
};

export function TrustBadge({ score, size = "md" }: TrustBadgeProps) {
  const tier = trustTier(score);
  const color = trustColor(score);
  return (
    <span title={tierTitle[tier]} className={clsx("inline-flex items-center gap-1 tabular-nums", size === "sm" ? "text-xs" : "text-sm")}>
      <span className={clsx("font-mono font-semibold", color)}>{score}</span>
      <span className="font-sans text-xs font-normal text-muted-foreground">{tierLabel[tier]}</span>
    </span>
  );
}
