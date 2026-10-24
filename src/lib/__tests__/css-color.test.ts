import { describe, expect, it } from "vitest";
import { blendOver, cssColorToRgb, rgbToHex } from "../css-color";

describe("blendOver", () => {
  it("returns the top color at alpha 1 and the bottom color at alpha 0", () => {
    expect(blendOver([10, 20, 30], [200, 210, 220], 1)).toEqual([10, 20, 30]);
    expect(blendOver([10, 20, 30], [200, 210, 220], 0)).toEqual([
      200, 210, 220,
    ]);
  });

  it("mixes per channel and rounds", () => {
    expect(blendOver([0, 0, 0], [255, 255, 255], 0.25)).toEqual([
      191, 191, 191,
    ]);
  });
});

describe("rgbToHex", () => {
  it("zero-pads each channel", () => {
    expect(rgbToHex([0, 15, 255])).toBe("#000fff");
  });
});

describe("cssColorToRgb", () => {
  it("returns null without a canvas context (jsdom)", () => {
    expect(cssColorToRgb("#ff0000")).toBeNull();
  });
});
