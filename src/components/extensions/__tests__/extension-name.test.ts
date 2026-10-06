import { describe, expect, it } from "vitest";
import { extensionDisplayName } from "../extension-name";

describe("extensionDisplayName", () => {
  it("leaves plugin-length names unchanged", () => {
    expect(extensionDisplayName("plugin", "cursor")).toBe("cursor");
    expect(extensionDisplayName("skill", "accessibility-kanban")).toBe(
      "accessibility-kanban",
    );
  });

  it("shortens event:matcher:command hooks and strips path tokens", () => {
    expect(
      extensionDisplayName(
        "hook",
        "Stop:*:/usr/bin/afplay /System/Library/Sounds/Glass.aiff",
      ),
    ).toBe("afplay Glass.aiff");
  });

  it("returns a bare hook one-liner unchanged (truncation is CSS)", () => {
    const raw =
      "if [ -f copilot-hook.sh ] && [ -x copilot-hook.sh ]; then sh copilot-hook.sh; fi";
    expect(extensionDisplayName("hook", raw)).toBe(raw);
  });

  it("returns a blob hook name unchanged", () => {
    const raw = `gBDXELA${"A".repeat(200)}`;
    expect(extensionDisplayName("hook", raw)).toBe(raw);
  });
});
