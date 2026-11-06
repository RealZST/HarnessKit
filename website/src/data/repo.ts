import repo from "./repo.json";

// Refreshed at build time by scripts/fetch-releases.mjs; the committed file is the offline fallback.
export const STARS: number = repo.stars;

const compact = new Intl.NumberFormat("en", { notation: "compact" });
// 467 → "467", 1234 → "1.2K", 15300 → "15K"
export const formatCount = (n: number) => compact.format(n);
