#!/usr/bin/env python3
"""

  LyMonS fontgen - it's worth the squeeze
  (c) 2025-26 Stuart Hunter

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
  - Digits (0-9) are right-justified with a 2px right margin.
  - Colon, minus, and space are horizontally centred.
  - If a glyph would exceed the tile width, it is horizontally scaled down
    to fit within (TILE_W - RIGHT_MARGIN - 1px) keeping right-justification,
    with a minimum 1px gap at the left ink edge.
  - Character path: fill #00ffff, stroke #ffffff at 0.5px opacity 0.4.
"""

import sys
import os
import zipfile
import subprocess

from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen


TILE_W   = 25   # tile width  (px)
TILE_H   = 44   # tile height (px)
BASELINE = 2    # px from bottom of tile to font baseline

CHARACTERS = [
    ('0', '0'), ('1', '1'), ('2', '2'), ('3', '3'), ('4', '4'),
    ('5', '5'), ('6', '6'), ('7', '7'), ('8', '8'), ('9', '9'),
    (':', 'colon'), (' ', 'space'), ('-', 'minus'),
]

# Reference character used to establish the shared scale
REFERENCE_CHAR = '8'


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


RIGHT_MARGIN   = 2      # px gap between glyph right ink edge and tile right edge
LEFT_GAP       = 1      # minimum px gap between glyph left ink edge and tile left edge
FILL_COLOR     = '#00ffff'
STROKE_COLOR   = '#ffffff'
STROKE_OPACITY = 0.4
STROKE_PX      = 0.5   # desired stroke width in SVG px

# Characters that are centred rather than right-justified
CENTRED_CHARS  = {':', ' ', '-'}


def make_svg(path_d, x_min, y_min, x_max, y_max, scale: float, char: str) -> str:
    """
    Build an SVG with the glyph path positioned using a shared scale and baseline.
      - Baseline sits BASELINE px from the bottom of the tile.
      - Digits are right-justified; colon, minus and space are centred.
      - If the glyph at the shared scale would be wider than the available
        horizontal space, it is scaled down horizontally only until it fits,
        preserving right-justification and maintaining a LEFT_GAP px minimum
        gap at the left ink edge. Vertical scale is always the shared scale.
      - y-axis is flipped (font = y-up, SVG = y-down).
      - Path style: fill FILL_COLOR, stroke STROKE_COLOR at STROKE_PX / 0.4 opacity.
      - Transparent tile background, no tile border.
    """
    if path_d is None:
        # Blank tile (space)
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'width="{TILE_W}" height="{TILE_H}" '
            f'viewBox="0 0 {TILE_W} {TILE_H}"></svg>'
        )

    glyph_units_w = x_max - x_min

    # Maximum rendered width before the left-gap constraint is violated
    if char in CENTRED_CHARS:
        max_glyph_px = TILE_W - 2 * LEFT_GAP          # equal margins both sides
    else:
        max_glyph_px = TILE_W - RIGHT_MARGIN - LEFT_GAP  # right-margin + left-gap

    # Clamp horizontal scale down if needed; never scale up beyond shared scale
    h_scale = scale
    if glyph_units_w > 0:
        max_h_scale = max_glyph_px / glyph_units_w
        if max_h_scale < scale:
            h_scale = max_h_scale

    glyph_px_w = glyph_units_w * h_scale

    if char in CENTRED_CHARS:
        # Centre the (possibly clamped) ink bounding box horizontally
        tx = (TILE_W - glyph_px_w) / 2 - x_min * h_scale
    else:
        # Right-justify: glyph right ink edge = TILE_W - RIGHT_MARGIN
        tx = (TILE_W - RIGHT_MARGIN) - x_max * h_scale

    # Vertical: font baseline (y=0) → svg_y = TILE_H - BASELINE; y-axis flipped
    # Vertical scale always uses the shared scale for consistent glyph heights.
    ty = TILE_H - BASELINE

    # Non-uniform scale only when h_scale was clamped; uniform otherwise.
    transform = f"translate({tx:.4f},{ty:.4f}) scale({h_scale:.6f},{-scale:.6f})"

    # stroke-width in font units so it renders as STROKE_PX after the vertical scale
    sw_font = STROKE_PX / scale

    style = (
        f'fill="{FILL_COLOR}" '
        f'stroke="{STROKE_COLOR}" '
        f'stroke-width="{sw_font:.4f}" '
        f'stroke-opacity="{STROKE_OPACITY}"'
    )

    clamped = " [width clamped]" if h_scale < scale else ""
    # (clamped flag available for callers; not used in SVG output)

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'width="{TILE_W}" height="{TILE_H}" '
        f'viewBox="0 0 {TILE_W} {TILE_H}">\n'
        f'  <path d="{path_d}" {style} transform="{transform}"/>\n'
        f'</svg>'
    ), h_scale < scale


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
        result = make_svg(path_d, x_min, y_min, x_max, y_max, scale, char)

        if isinstance(result, tuple):
            svg_content, clamped = result
        else:
            svg_content, clamped = result, False

        filename = f"{seed}_{label}.svg"
        filepath = os.path.join(output_dir, filename)
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(svg_content)

        generated_files.append(filepath)
        if path_d is None:
            status = "blank (no glyph)"
        elif clamped:
            status = "path rendered [width clamped to fit]"
        else:
            status = "path rendered"
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