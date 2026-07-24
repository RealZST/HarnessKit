#!/usr/bin/env node
// Build the desktop (latest.json) variant of the release notes.
//
// The canonical release body keeps translations inside HTML comment blocks
// (`<!-- lang:zh ... -->`) so the GitHub release page renders English only.
// Desktop clients ≤1.7.0 only understand the legacy fence format
// (`<!-- lang:en -->` / `<!-- lang:zh -->`), so for latest.json this script
// converts comment blocks back into fences:
//
//   <!-- lang:en -->
//   <plain-text notes + What's Changed>
//   <!-- lang:zh -->
//   <translation>
//
// The translation section goes LAST and the "## New Contributors" /
// "**Full Changelog**" tails are dropped: old cleanChangelog skips from those
// lines until the next `## ` heading, which would swallow a following
// `<!-- lang:xx -->` fence and concatenate both languages (the v1.7.0 bug).
// The updater dialog hides those sections anyway.
//
// Usage: node scripts/notes-desktop-variant.mjs <body-file>
// Reads the full release body, writes the desktop variant to stdout.
// Bodies without a comment block pass through unchanged.

import { readFileSync } from "node:fs";

const LANG_COMMENT_BLOCK = /<!--\s*lang:([a-z-]+)[ \t]*\n([\s\S]*?)-->/gi;

const body = readFileSync(process.argv[2], "utf8");

const translations = [];
let remainder = body
  .replace(LANG_COMMENT_BLOCK, (_match, code, content) => {
    translations.push({ code: code.toLowerCase(), content: content.trim() });
    return "";
  })
  .trim();

if (translations.length === 0) {
  process.stdout.write(body.trim() + "\n");
  process.exit(0);
}

// Drop the tail sections that cleanChangelog would hide anyway — they must
// not precede a fence (see header comment).
const lines = [];
let skip = false;
for (const line of remainder.split("\n")) {
  if (
    line.startsWith("## New Contributors") ||
    line.startsWith("**Full Changelog**")
  ) {
    skip = true;
    continue;
  }
  if (skip && line.startsWith("## ")) skip = false;
  if (skip) continue;
  lines.push(line);
}
remainder = lines.join("\n").trim();

const parts = [`<!-- lang:en -->\n${remainder}`];
for (const { code, content } of translations) {
  parts.push(`<!-- lang:${code} -->\n${content}`);
}
process.stdout.write(parts.join("\n\n") + "\n");
