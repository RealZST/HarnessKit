const EN = ["One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Eleven", "Twelve",
  "Thirteen", "Fourteen", "Fifteen", "Sixteen", "Seventeen", "Eighteen", "Nineteen", "Twenty"] as const;

/** A count for a headline: "Seventeen" in English for 1–20, digits otherwise and always in Chinese. */
export function countInWords(n: number, lang: "en" | "zh"): string {
  if (lang === "en" && Number.isInteger(n) && n >= 1 && n <= EN.length) return EN[n - 1];
  return String(n);
}
