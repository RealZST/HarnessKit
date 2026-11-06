/** Plain or `code` text. Link text is made of these, so a link can contain code. */
export interface InlineText { kind: "text" | "code"; text: string }
/** One run of copy text: plain, a `code` span, or a [link](href). */
export type InlineSegment = InlineText | { kind: "link"; href: string; parts: InlineText[] };

const CODE = "`([^`]+)`";
// Link text has no brackets, so in "[a [b](c)" only "b" links. The href stops at its matching
// paren and may hold one nested pair, as in Wikipedia's Foo_(bar).
const LINK = String.raw`\[([^\[\]]+)\]\(((?:[^()\s]|\([^()\s]*\))+)\)`;
const INLINE = new RegExp(`${CODE}|${LINK}`, "g");
const CODE_ONLY = new RegExp(CODE, "g");
// Root-relative (not protocol-relative "//"), in-page, or https. Anything else is a copy mistake.
const SAFE_HREF = /^(?:\/(?!\/)|#|https:\/\/)/;

function splitCode(text: string): InlineText[] {
  const parts: InlineText[] = [];
  let last = 0;
  for (const m of text.matchAll(CODE_ONLY)) {
    if (m.index > last) parts.push({ kind: "text", text: text.slice(last, m.index) });
    parts.push({ kind: "code", text: m[1] });
    last = m.index + m[0].length;
  }
  if (last < text.length) parts.push({ kind: "text", text: text.slice(last) });
  return parts;
}

/** Splits copy text on its inline markup. Throws on an href that isn't /…, #… or https://…, failing the build. */
export function renderInline(text: string): InlineSegment[] {
  const segments: InlineSegment[] = [];
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index > last) segments.push({ kind: "text", text: text.slice(last, m.index) });
    if (m[1] !== undefined) {
      segments.push({ kind: "code", text: m[1] });
    } else {
      const href = m[3];
      if (!SAFE_HREF.test(href)) throw new Error(`renderInline: href must start with /, # or https:// — got "${href}" in "${text}"`);
      segments.push({ kind: "link", href, parts: splitCode(m[2]) });
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) segments.push({ kind: "text", text: text.slice(last) });
  return segments;
}
