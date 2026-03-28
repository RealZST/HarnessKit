import { trustTier, trustColor } from "@/lib/types";
import { clsx } from "clsx";

interface TrustBadgeProps {
  score: number;
  size?: "sm" | "md";
}

const dotColor: Record<string, string> = {
  Safe: "bg-primary",
  LowRisk: "bg-chart-4",
  HighRisk: "bg-chart-5",
  Critical: "bg-destructive",
};

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
    <span title={tierTitle[tier]} className={clsx("inline-flex items-center gap-1.5 font-mono font-semibold tabular-nums", color, size === "sm" ? "text-xs" : "text-sm")}>
      <span aria-hidden="true" className={clsx("inline-block shrink-0 rounded-full", dotColor[tier], size === "sm" ? "size-1.5" : "size-2")} />
      {score}
      <span className="hidden font-sans text-xs font-normal text-muted-foreground sm:inline">{tierLabel[tier]}</span>
    </span>
  );
}
