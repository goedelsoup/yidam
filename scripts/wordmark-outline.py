#!/usr/bin/env python3
"""Set a word in a font and print it as SVG path data.

Run by hand, not in CI, and not by the generator. The whole reason the wordmark is outlined
is that the shipped asset must not depend on a font being present: an SVG inside an <img> is
an isolated document that cannot reach the host page's webfonts, so the old
`<text font-family="Cormorant Garamond">` wordmark rendered in whatever serif the reader
happened to have — which on the machine that first measured it was not Cormorant.

So the font is a build-time input exactly once, here, and its output is committed into
`scripts/mark-generate.mjs` as `WORDMARK_PATH`.

    $ python3 scripts/wordmark-outline.py ~/fonts/Spectral-Regular.ttf yidam 100 0.04

Kerning is not applied. GPOS is not read here, and at the tracking the wordmark uses the
pairs in "yidam" carry no kerning worth the dependency. Check the output by eye if the word
ever changes.

Needs fontTools:  python3 -m venv .venv && .venv/bin/pip install fonttools
"""

import re
import sys

from fontTools.misc.transform import Transform
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont


def outline(font_path, word, size, tracking_em=0.0):
    """Return (path_data, ink_bounds) with the baseline at y=0 and the em at `size` units."""
    font = TTFont(font_path)
    upm = font["head"].unitsPerEm
    cmap = font.getBestCmap()
    glyphs = font.getGlyphSet()
    hmtx = font["hmtx"]

    scale = size / upm
    track = tracking_em * size
    bounds = BoundsPen(glyphs)
    parts = []
    x = 0.0

    for ch in word:
        name = cmap[ord(ch)]
        # Font space is y-up and SVG is y-down, so the y scale is negated.
        transform = Transform(scale, 0, 0, -scale, x, 0)
        pen = SVGPathPen(glyphs)
        glyphs[name].draw(TransformPen(pen, transform))
        commands = pen.getCommands()
        if commands:
            parts.append(commands)
        glyphs[name].draw(TransformPen(bounds, transform))
        x += hmtx[name][0] * scale + track

    return " ".join(parts), bounds.bounds


def round_path(data, places=2):
    """Two decimals at em=100 is under 0.01px of error at any size this is ever drawn."""
    rounded = re.sub(
        r"-?\d+\.\d+",
        lambda m: f"{float(m.group()):.{places}f}".rstrip("0").rstrip("."),
        data,
    )
    return re.sub(r"\s+", " ", rounded).strip()


def main():
    if len(sys.argv) != 5:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    font_path, word, size, tracking = sys.argv[1:]
    data, (x0, y0, x1, y1) = outline(font_path, word, float(size), float(tracking))
    data = round_path(data)
    print(f"// ink bounds: x {x0:.1f}..{x1:.1f}  y {y0:.1f}..{y1:.1f}", file=sys.stderr)
    print(f"// {len(data)} chars", file=sys.stderr)
    print(data)
    return 0


if __name__ == "__main__":
    sys.exit(main())
