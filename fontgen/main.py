#!/usr/bin/env python3
"""
generate_tiles.py - Generate SVG tiles for characters "0123456789: -"

Usage:
    python3 generate_tiles.py <font_name_or_path> <seed>

Arguments:
    font_name_or_path  - Font name (e.g. "DejaVu Sans") or path to .ttf/.otf file
    seed               - Seed name used for output filenames and zip

Output:
    <seed>_0.svg .. <seed>_9.svg, <seed>_colon.svg, <seed>_space.svg, <seed>_minus.svg
    <seed>.zip  - containing all SVGs

Sizing rules:
  - All glyphs share the same scale, derived from the digit '8'.
  - Colon and minus are rendered at natural size within that shared scale
    (i.e. as they would appear alongside digits, not stretched to fill the tile).
  - All glyphs share a common baseline 2px from the bottom of the tile.
  - Glyphs are horizontally centred within the tile.
"""

import sys
import os
import zipfile
import subprocess

from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen


TILE_W   = 24   # tile width  (px)
TILE_H   = 40   # tile height (px)
BASELINE = 2    # px from bottom of tile to font baseline

CHARACTERS = [
    ('0', '0'), ('1', '1'), ('2', '2'), ('3', '3'), ('4', '4'),
    ('5', '5'), ('6', '6'), ('7', '7'), ('8', '8'), ('9', '9'),
    (':', 'colon'), (' ', 'space'), ('-', 'minus'),
]

# Reference character used to establish the shared scale
REFERENCE_CHAR = '8'  # in the good old days, we'd have used M or W - ah memories...


def find_font_path(name: str) -> str:
    """Resolve a font name or path to an actual file path."""
    if os.path.isfile(name):
        return name
    try:
        result = subprocess.run(
            ['fc-match', '--format=%{file}', name],
            capture_output=True, text=True, check=True
        )
        path = result.stdout.strip()
        if path and os.path.isfile(path):
            return path
    except Exception:
        pass
    raise FileNotFoundError(
        f"Cannot find font '{name}'. "
        "Provide a full path to a .ttf/.otf file, or a valid fc-match name."
    )


def get_glyph_info(tt: TTFont, char: str):
    """
    Return (path_d, x_min, y_min, x_max, y_max) in font units.
    path_d is None for missing / empty glyphs (e.g. space).
    """
    cmap      = tt.getBestCmap()
    glyph_set = tt.getGlyphSet()
    cp        = ord(char)

    glyph_name = cmap.get(cp) if cmap else None

    path_d = None
    if glyph_name and glyph_name in glyph_set:
        pen = SVGPathPen(glyph_set)
        glyph_set[glyph_name].draw(pen)
        path_d = pen.getCommands() or None

    # Bounding box
    bounds = None
    if glyph_name and 'glyf' in tt:
        g = tt['glyf'][glyph_name]
        if hasattr(g, 'xMin'):
            bounds = (g.xMin, g.yMin, g.xMax, g.yMax)

    if bounds is None:
        ascender  = tt['OS/2'].sTypoAscender
        descender = tt['OS/2'].sTypoDescender
        adv = tt['hmtx'].metrics.get(glyph_name or '', (500, 0))[0]
        bounds = (0, descender, adv, ascender)

    return path_d, bounds[0], bounds[1], bounds[2], bounds[3]


def compute_scale(tt: TTFont) -> float:
    """
    Compute a single px-per-font-unit scale so that the reference character ('8')
    fits within the available tile height (TILE_H - BASELINE - top_margin).
    All characters use this same scale, preserving true typographic proportions.
    """
    _, _, y_min, _, y_max = get_glyph_info(tt, REFERENCE_CHAR)
    glyph_h    = y_max - y_min
    top_margin = 2  # px of breathing room above the tallest digit
    avail_h    = TILE_H - BASELINE - top_margin
    return avail_h / glyph_h


def make_svg(path_d, x_min, y_min, x_max, y_max, scale: float) -> str:
    """
    Build an SVG with the glyph path positioned using a shared scale and baseline.
      - Baseline sits BASELINE px from the bottom of the tile.
      - Glyph is horizontally centred within the tile.
      - y-axis is flipped (font = y-up, SVG = y-down).
      - Fill #ffffff on a transparent background.
    """
    if path_d is None:
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'width="{TILE_W}" height="{TILE_H}" '
            f'viewBox="0 0 {TILE_W} {TILE_H}"></svg>'
        )

    glyph_w = (x_max - x_min) * scale

    # Horizontal: centre the glyph bounding box in the tile
    tx = (TILE_W - glyph_w) / 2 - x_min * scale

    # Vertical: font baseline (y=0) maps to svg_y = TILE_H - BASELINE; y flipped
    ty = TILE_H - BASELINE

    transform = f"translate({tx:.4f},{ty:.4f}) scale({scale:.6f},{-scale:.6f})"

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'width="{TILE_W}" height="{TILE_H}" '
        f'viewBox="0 0 {TILE_W} {TILE_H}">\n'
        f'  <path d="{path_d}" fill="#ffffff" transform="{transform}"/>\n'
        f'</svg>'
    )


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)

    font_arg = sys.argv[1]
    seed     = sys.argv[2]

    print(f"Resolving font: {font_arg!r} …")
    font_path = find_font_path(font_arg)
    print(f"Using font file: {font_path}")

    tt    = TTFont(font_path)
    scale = compute_scale(tt)
    print(f"Shared scale: {scale:.6f} px/font-unit  (baseline: {BASELINE}px from bottom)")

    output_dir      = os.getcwd()
    generated_files = []

    for char, label in CHARACTERS:
        path_d, x_min, y_min, x_max, y_max = get_glyph_info(tt, char)
        svg_content = make_svg(path_d, x_min, y_min, x_max, y_max, scale)

        filename = f"{seed}_{label}.svg"
        filepath = os.path.join(output_dir, filename)
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(svg_content)

        generated_files.append(filepath)
        status = "path rendered" if path_d else "blank (no glyph)"
        print(f"  {filename}  [{status}]")

    # Zip all SVGs
    zip_name = f"{seed}.zip"
    zip_path = os.path.join(output_dir, zip_name)
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
        for fp in generated_files:
            zf.write(fp, os.path.basename(fp))

    # Remove individual SVG files now that they're archived
    for fp in generated_files:
        os.remove(fp)

    print(f"\nDone! {len(generated_files)} SVGs zipped → {zip_path}")


if __name__ == '__main__':
    main()