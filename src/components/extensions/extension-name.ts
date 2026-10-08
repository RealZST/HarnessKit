import type { ExtensionKind } from "@/lib/types";

/** Friendly label for an `event:matcher:command` hook: the command with
 *  directory paths stripped ("/usr/bin/afplay /S/L/Sounds/Glass.aiff" →
 *  "afplay Glass.aiff"). View-layer only. Sort, filter, and identity use raw
 *  `ext.name`. */
export function extensionDisplayName(
  kind: ExtensionKind,
  name: string,
): string {
  if (kind === "hook") {
    const parts = name.split(":");
    if (parts.length >= 3) {
      const cmd = parts.slice(2).join(":");
      return cmd
        .split(" ")
        .map((t) => t.split("/").pop() || t)
        .join(" ");
    }
  }
  return name;
}

/** Event segment of an `event:matcher:command` hook name; null for names
 *  stored in any other shape (a bare one-liner is not an event). */
export function hookEventName(name: string): string | null {
  const parts = name.split(":");
  return parts.length >= 3 ? parts[0] : null;
}
