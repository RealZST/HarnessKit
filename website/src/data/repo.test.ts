import { describe, expect, it } from "vitest";
import { formatCount } from "./repo";

describe("formatCount", () => {
  it("keeps small counts as written", () => {
    expect(formatCount(467)).toBe("467");
  });
  it("abbreviates thousands the way GitHub does", () => {
    expect(formatCount(1234)).toBe("1.2K");
    expect(formatCount(15_300)).toBe("15K");
  });
});
