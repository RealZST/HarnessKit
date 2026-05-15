import { clsx } from "clsx";
import { useTranslation } from "react-i18next";
import { trustColor, trustTier } from "@/lib/types";

interface TrustBadgeProps {
  score: number;
  size?: "sm" | "md";
}

const TIER_TITLE_KEY = {
  Safe: "tiers.safeTitle",
  LowRisk: "tiers.lowRiskTitle",
  NeedsReview: "tiers.needsReviewTitle",
} as const;

export function TrustBadge({ score, size = "md" }: TrustBadgeProps) {
  const { t } = useTranslation("audit");
  const tier = trustTier(score);
  const color = trustColor(score);
  return (
    <span
      title={`${t(`tiers.${tier}`)} — ${t(TIER_TITLE_KEY[tier])}`}
      className={clsx(
        "font-mono font-semibold tabular-nums",
        color,
        size === "sm" ? "text-xs" : "text-sm",
      )}
    >
      {score}
    </span>
  );
}
