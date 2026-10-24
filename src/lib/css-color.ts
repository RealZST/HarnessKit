/**
 * Color helpers for handing CSS theme colors to native code.
 */

export type Rgb = readonly [number, number, number];

/**
 * Resolve any CSS color (the theme uses oklch) to sRGB bytes by painting it
 * on a 1×1 canvas. Returns null when the color does not parse or no canvas
 * is available (jsdom).
 */
export function cssColorToRgb(color: string): Rgb | null {
  const ctx = document.createElement("canvas").getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
  return a === 0 ? null : [r, g, b];
}

/** `top` composited over `bottom` at `alpha` (0–1), per channel. */
export function blendOver(top: Rgb, bottom: Rgb, alpha: number): Rgb {
  const mix = (t: number, b: number) => Math.round(t * alpha + b * (1 - alpha));
  return [
    mix(top[0], bottom[0]),
    mix(top[1], bottom[1]),
    mix(top[2], bottom[2]),
  ];
}

export function rgbToHex([r, g, b]: Rgb): string {
  const hex = (n: number) => n.toString(16).padStart(2, "0");
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}
