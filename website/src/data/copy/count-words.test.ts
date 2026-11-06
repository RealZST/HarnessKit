// @vitest-environment node
import { describe, expect, it } from "vitest";
import { countInWords } from "./count-words";

describe("countInWords", () => {
  it("spells 1 to 20 as capitalised English words", () => {
    expect(countInWords(1, "en")).toBe("One");
    expect(countInWords(17, "en")).toBe("Seventeen");
    expect(countInWords(20, "en")).toBe("Twenty");
  });
  it("uses digits above 20, and for zero or non-integers", () => {
    expect(countInWords(21, "en")).toBe("21");
    expect(countInWords(0, "en")).toBe("0");
    expect(countInWords(2.5, "en")).toBe("2.5");
  });
  it("uses digits for Chinese", () => {
    expect(countInWords(17, "zh")).toBe("17");
  });
});
