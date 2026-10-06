import type { ExtensionKind } from "@/lib/types";

/** View-layer only. Sort, filter, and identity use raw `ext.name`. */
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
