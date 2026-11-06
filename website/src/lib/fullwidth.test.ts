// @vitest-environment node
import { describe, expect, it } from "vitest";
import { splitTrailingQuestion } from "./fullwidth";

describe("splitTrailingQuestion", () => {
  it("splits a trailing full-width question mark off", () => {
    expect(splitTrailingQuestion("Intel 芯片？")).toEqual(["Intel 芯片", "？"]);
  });
  it("keeps a half-width question mark with its text", () => {
    expect(splitTrailingQuestion("Intel Mac?")).toEqual(["Intel Mac?", ""]);
  });
});
