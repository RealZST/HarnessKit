import { describe, expect, it } from "vitest";
import { formatBytes } from "../format";

describe("formatBytes", () => {
  it("uses B under 1KB", () => expect(formatBytes(512)).toBe("512 B"));
  it("uses one-decimal KB at/above 1KB", () =>
    expect(formatBytes(1536)).toBe("1.5 KB"));
  it("handles zero", () => expect(formatBytes(0)).toBe("0 B"));
});
