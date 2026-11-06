// Chinese punctuation is full-width: the mark takes half of its em and the other half is empty. This helper keeps a
// trailing mark out of a link, whose underline would run under the empty half.

/**
 * Splits a trailing full-width question mark off: "Intel 芯片？" → ["Intel 芯片", "？"]; other text → [text, ""].
 * Templates set the mark in a `.fw-q` span (InstallOptions.astro), which takes its empty half back.
 */
export function splitTrailingQuestion(text: string): [string, string] {
  return text.endsWith("？") ? [text.slice(0, -1), "？"] : [text, ""];
}
