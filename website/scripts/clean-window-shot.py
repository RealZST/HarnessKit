# Cuts the HarnessKit window out of a screenshot of it on a desktop (CleanShot and the like) for the site's pictures:
# crops just inside the macOS window outline, repaints the corner rim with the window's own background (the outline
# there is mixed with the desktop), masks the corners with an anti-aliased rounded rectangle, and draws a faint
# hairline round the edge as macOS does. No shadow: the site adds its own.
#
#   python3 scripts/clean-window-shot.py <capture.png> <out.png> <left> <top> <right> <bottom> <radius>
#
# left/top/right/bottom: the first and last pixels of the window's content inside the outline, in the capture's
# pixels; radius: the corner radius in the same pixels (36 for a 2x capture on macOS 26). Example, 2026-11-05:
#   python3 scripts/clean-window-shot.py capture.png src/assets/screens/overview.png 122 122 2643 1721 36
import math
import sys

from PIL import Image, ImageChops, ImageDraw

WINDOW_BG = (239, 242, 247, 255)
SUPERSAMPLE = 4
HAIRLINE = (20, 30, 45)
HAIRLINE_ALPHA = 0.16


def is_window_bg(p):
    return min(p[:3]) > 225 and (p[2] - p[0]) < 14


def repaint_corner_rims(img, r):
    """Paints over the outline and desktop left at each corner, and along the first rows next to it."""
    w, h = img.size
    px = img.load()
    reach = int(r) + 10
    for right in (False, True):
        for bottom in (False, True):
            cx = w - r if right else r
            cy = h - r if bottom else r
            for y in range(h - reach, h) if bottom else range(reach):
                for x in range(w - reach, w) if right else range(reach):
                    past_x = x + 0.5 > cx if right else x + 0.5 < cx
                    past_y = y + 0.5 > cy if bottom else y + 0.5 < cy
                    on_rim = past_x and past_y and math.hypot(x + 0.5 - cx, y + 0.5 - cy) > r - 8
                    near_edge = min(x, y, w - 1 - x, h - 1 - y) < 3
                    if (on_rim or near_edge) and not is_window_bg(px[x, y]):
                        px[x, y] = WINDOW_BG


def rounded_mask(size, r, inset=0.0):
    w, h = size
    m = Image.new("L", (w * SUPERSAMPLE, h * SUPERSAMPLE), 0)
    box = (inset * SUPERSAMPLE, inset * SUPERSAMPLE, (w - inset) * SUPERSAMPLE - 1, (h - inset) * SUPERSAMPLE - 1)
    ImageDraw.Draw(m).rounded_rectangle(box, radius=(r - inset) * SUPERSAMPLE, fill=255)
    return m.resize((w, h), Image.LANCZOS)


def main():
    src, out = sys.argv[1], sys.argv[2]
    left, top, right, bottom = (int(v) for v in sys.argv[3:7])
    r = float(sys.argv[7])
    window = Image.open(src).convert("RGBA").crop((left, top, right + 1, bottom + 1))
    repaint_corner_rims(window, r)
    mask = rounded_mask(window.size, r)
    ring = ImageChops.subtract(mask, rounded_mask(window.size, r, inset=1.25))
    line = Image.new("RGBA", window.size, HAIRLINE + (0,))
    line.putalpha(ring.point(lambda v: int(v * HAIRLINE_ALPHA)))
    result = Image.alpha_composite(window, line)
    result.putalpha(mask)
    result.save(out, optimize=True)
    print(f"{out}: {result.size[0]}×{result.size[1]}")


if __name__ == "__main__":
    main()
