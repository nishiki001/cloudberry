#!/usr/bin/env python3
"""Generates assets/icons/{aero,pixel}/*.svg and manifest.json from the glyph table below.
The `line` set is the Lucide set (ISC, LICENSE-lucide.txt), copied under the canonical names.
Run from the repo root: python3 assets/icons/gen_icons.py
Glyphs live on a 16x16 grid; primitives: P polygon, R rect, C disc, O ring, L round line."""
import json, os, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
P = lambda *pts: ("P", pts)
R = lambda x, y, w, h: ("R", x, y, w, h)
C = lambda x, y, r: ("C", x, y, r)
O = lambda x, y, r, t: ("O", x, y, r, t)
L = lambda x1, y1, x2, y2, t=1.6: ("L", x1, y1, x2, y2, t)
SPK = P((2, 6), (5, 6), (8, 3), (8, 13), (5, 10), (2, 10))

# name: (lucide source file or None, aero hue, glyph)
G = {
    "play": ("play", 130, [P((5, 3), (13, 8), (5, 13))]),
    "pause": ("pause", 205, [R(4, 3, 3, 10), R(9, 3, 3, 10)]),
    "stop": ("square", 8, [R(4, 4, 8, 8)]),
    "prev": ("skip-back", 205, [R(3, 3, 2, 10), P((13, 3), (5, 8), (13, 13))]),
    "next": ("skip-forward", 205, [R(11, 3, 2, 10), P((3, 3), (11, 8), (3, 13))]),
    "shuffle": ("shuffle", 280, [L(2, 4, 5, 4), L(5, 4, 10, 12), L(10, 12, 12, 12), L(2, 12, 5, 12), L(5, 12, 10, 4), L(10, 4, 12, 4), P((11, 2), (14, 4), (11, 6)), P((11, 10), (14, 12), (11, 14))]),
    "repeat": ("repeat", 280, [L(3, 9, 3, 6), L(3, 6, 4, 5), L(4, 5, 11, 5), P((10, 2), (13, 5), (10, 8)), L(13, 7, 13, 10), L(13, 10, 12, 11), L(12, 11, 5, 11), P((6, 8), (3, 11), (6, 14))]),
    "repeat-one": ("repeat-1", 280, [L(3, 9, 3, 6), L(3, 6, 4, 5), L(4, 5, 11, 5), P((10, 2), (13, 5), (10, 8)), L(13, 7, 13, 10), L(13, 10, 12, 11), L(12, 11, 5, 11), P((6, 8), (3, 11), (6, 14)), R(7.2, 6.6, 1.6, 3.6)]),
    "volume": ("volume-2", 175, [SPK, L(11, 6, 11, 10), L(13.4, 4, 13.4, 12)]),
    "volume-low": ("volume-1", 175, [SPK, L(11, 6, 11, 10)]),
    "mute": ("volume-x", 8, [SPK, L(10.5, 6, 14, 10), L(14, 6, 10.5, 10)]),
    "heart": ("heart", 345, [C(5.5, 6, 3.2), C(10.5, 6, 3.2), P((2.6, 7.2), (13.4, 7.2), (8, 13.5))]),
    "search": ("search", 215, [O(6.5, 6.5, 4, 1.7), L(9.6, 9.6, 13.4, 13.4, 2.2)]),
    "library": ("library", 30, [R(3, 3, 2, 10), R(6.5, 3, 2, 10), P((10, 3.6), (12, 3), (14, 12.4), (12, 13))]),
    "now": ("disc-3", 195, [O(8, 8, 5.4, 1.7), C(8, 8, 1.6)]),
    "lists": ("list-music", 255, [L(2, 4, 10, 4), L(2, 8, 10, 8), L(2, 12, 7, 12), R(13, 5, 1.3, 6.5), C(11.8, 11.5, 1.8)]),
    "discover": ("compass", 160, [O(8, 8, 5.6, 1.5), P((10.8, 5.2), (9.2, 9.2), (5.2, 10.8), (6.8, 6.8))]),
    "settings": ("settings", 220, [O(8, 8, 4, 2), R(7, 1.5, 2, 2.6), R(7, 11.9, 2, 2.6), R(1.5, 7, 2.6, 2), R(11.9, 7, 2.6, 2), L(3.6, 3.6, 4.7, 4.7, 2), L(12.4, 3.6, 11.3, 4.7, 2), L(3.6, 12.4, 4.7, 11.3, 2), L(12.4, 12.4, 11.3, 11.3, 2)]),
    "add": ("plus", 130, [R(7, 3, 2, 10), R(3, 7, 10, 2)]),
    "playlist-add": ("list-plus", 130, [L(2, 4, 10, 4), L(2, 8, 7, 8), L(2, 12, 10, 12), R(11.5, 6, 1.6, 7), R(9, 8.7, 6.6, 1.6)]),
    "close": ("x", 8, [L(4, 4, 12, 12, 2), L(12, 4, 4, 12, 2)]),
    "back": ("chevron-left", 215, [L(10, 3, 5, 8, 2), L(5, 8, 10, 13, 2)]),
    "forward": ("chevron-right", 215, [L(6, 3, 11, 8, 2), L(11, 8, 6, 13, 2)]),
    "down": ("chevron-down", 215, [L(3, 6, 8, 11, 2), L(8, 11, 13, 6, 2)]),
    "sort-up": ("arrow-up", 215, [P((8, 4), (12.5, 10), (3.5, 10))]),
    "sort-down": ("arrow-down", 215, [P((8, 12), (12.5, 6), (3.5, 6))]),
    "more": ("ellipsis", 215, [C(3.5, 8, 1.5), C(8, 8, 1.5), C(12.5, 8, 1.5)]),
    "refresh": ("refresh-cw", 175, [O(8, 8, 4.8, 1.7), P((10.6, 1.5), (14.6, 5.2), (9.6, 6.2))]),
    "music": ("music", 300, [C(5, 12, 2.4), R(6.8, 3, 1.5, 9.5), P((8.3, 3), (13, 4.6), (13, 7.2), (8.3, 5.6))]),
    "lyrics": ("mic-vocal", 300, [R(6, 2.2, 4, 7), L(4, 8, 4, 9), L(4, 9, 6, 11.5), L(6, 11.5, 10, 11.5), L(10, 11.5, 12, 9), L(12, 9, 12, 8), L(8, 11.5, 8, 14), L(5.5, 14, 10.5, 14)]),
    "history": ("clock", 195, [O(8, 8, 5.5, 1.5), L(8, 8, 8, 4.6, 1.4), L(8, 8, 10.5, 9.5, 1.4)]),
    "user": ("user", 215, [C(8, 5, 2.8), P((3, 14), (3.5, 11), (6, 9.5), (10, 9.5), (12.5, 11), (13, 14))]),
    "list": ("list", 215, [L(3, 4, 13, 4), L(3, 8, 13, 8), L(3, 12, 13, 12)]),
    "check": ("check", 130, [L(3, 8.5, 6.5, 12, 2), L(6.5, 12, 13, 4.5, 2)]),
    "equalizer": ("sliders-horizontal", 220, [R(3, 8, 2, 5), R(7, 4, 2, 9), R(11, 6, 2, 7)]),
    "no-image": ("image-off", 215, [R(2, 3, 12, 1.4), R(2, 11.6, 12, 1.4), R(2, 3, 1.4, 10), R(12.6, 3, 1.4, 10), L(3, 13, 13, 3, 1.4)]),
    "folder": ("folder-open", 40, [P((2, 4), (6, 4), (7.5, 5.5), (14, 5.5), (14, 13), (2, 13))]),
}


def dist_seg(px, py, x1, y1, x2, y2):
    dx, dy = x2 - x1, y2 - y1
    t = 0 if dx == dy == 0 else max(0, min(1, ((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy)))
    return ((px - x1 - t * dx) ** 2 + (py - y1 - t * dy) ** 2) ** 0.5


def inside(g, px, py):
    k = g[0]
    if k == "R":
        return g[1] <= px <= g[1] + g[3] and g[2] <= py <= g[2] + g[4]
    if k == "C":
        return (px - g[1]) ** 2 + (py - g[2]) ** 2 <= g[3] ** 2
    if k == "O":
        d = ((px - g[1]) ** 2 + (py - g[2]) ** 2) ** 0.5
        return abs(d - g[3]) <= g[4] / 2
    if k == "L":
        return dist_seg(px, py, g[1], g[2], g[3], g[4]) <= g[5] / 2
    pts, n, c = g[1], len(g[1]), False
    for i in range(n):
        (x1, y1), (x2, y2) = pts[i], pts[(i + 1) % n]
        if (y1 > py) != (y2 > py) and px < (x2 - x1) * (py - y1) / (y2 - y1) + x1:
            c = not c
    return c


def vec(g, fill, extra=""):
    k = g[0]
    f = f'fill="{fill}"'
    if k == "R":
        return f'<rect x="{g[1]}" y="{g[2]}" width="{g[3]}" height="{g[4]}" {f}{extra}/>'
    if k == "C":
        return f'<circle cx="{g[1]}" cy="{g[2]}" r="{g[3]}" {f}{extra}/>'
    if k == "O":
        return f'<circle cx="{g[1]}" cy="{g[2]}" r="{g[3]}" fill="none" stroke="{fill}" stroke-width="{g[4]}"{extra}/>'
    if k == "L":
        return f'<line x1="{g[1]}" y1="{g[2]}" x2="{g[3]}" y2="{g[4]}" stroke="{fill}" stroke-width="{g[5]}" stroke-linecap="round"{extra}/>'
    pts = " ".join(f"{x},{y}" for x, y in g[1])
    return f'<polygon points="{pts}" {f} stroke="{fill}" stroke-width="0.6" stroke-linejoin="round"{extra}/>'


def hsl(h, s, l):
    import colorsys
    r, g, b = colorsys.hls_to_rgb(h / 360, l, s)
    return "#%02x%02x%02x" % (round(r * 255), round(g * 255), round(b * 255))


def aero(name, hue, glyph):
    top, bot, rim = hsl(hue, 0.85, 0.66), hsl(hue, 0.9, 0.36), hsl(hue, 0.8, 0.28)
    d = f'<defs><linearGradient id="b" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bot}"/></linearGradient>' \
        '<linearGradient id="h" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.9"/><stop offset="1" stop-color="#fff" stop-opacity="0.1"/></linearGradient>' \
        f'<radialGradient id="g" cx="0.5" cy="1" r="0.7"><stop offset="0" stop-color="{hsl(hue, 1, 0.8)}" stop-opacity="0.8"/><stop offset="1" stop-color="{hsl(hue, 1, 0.8)}" stop-opacity="0"/></radialGradient></defs>'
    body = '<circle cx="12" cy="12" r="11.2" fill="url(#b)" stroke="%s" stroke-width="0.8"/>' % rim
    body += '<circle cx="12" cy="12" r="11.2" fill="url(#g)"/>'
    body += '<ellipse cx="12" cy="7.2" rx="8.4" ry="5.4" fill="url(#h)"/>'
    body += '<circle cx="12" cy="12" r="10.4" fill="none" stroke="#fff" stroke-opacity="0.55" stroke-width="0.7"/>'
    sh = "".join(vec(g, "#000", ' opacity="0.28"') for g in glyph)
    fg = "".join(vec(g, "#fff") for g in glyph)
    body += f'<g transform="translate(4.2,4.7) scale(0.95)">{sh}</g><g transform="translate(4,4) scale(0.95)">{fg}</g>'
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{d}{body}</svg>\n'


# ---- Pixel set: hand-placed integer shapes on a 14x14 canvas (1 px margin on the 16 grid for the
# dark outline). Filled silhouette + 1 px outline + a lighter top edge; flat colour per icon.
def _tri(x, y, w, h, d):
    """Triangle in the box; d = r (points right), l, u, d."""
    out = set()
    for i in range(h if d in "rl" else w):
        n = (h if d in "rl" else w)
        k = 1 + int((1 - abs(2 * i + 1 - n) / n) * (w if d in "rl" else h) + 0.5)
        k = min(k, w if d in "rl" else h)
        for j in range(k):
            if d == "r": out.add((x + j, y + i))
            if d == "l": out.add((x + w - 1 - j, y + i))
            if d == "u": out.add((x + i, y + h - 1 - j))
            if d == "d": out.add((x + i, y + j))
    return out


def _rect(x, y, w, h):
    return {(x + i, y + j) for i in range(w) for j in range(h)}


def _disc(cx, cy, r):
    return {(x, y) for x in range(14) for y in range(14) if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + r * 0.6}


def _ring(cx, cy, r, t):
    return _disc(cx, cy, r) - _disc(cx, cy, r - t)


def _line(x1, y1, x2, y2, t=2):
    out, n = set(), max(abs(x2 - x1), abs(y2 - y1), 1)
    for i in range(n + 1):
        x, y = round(x1 + (x2 - x1) * i / n), round(y1 + (y2 - y1) * i / n)
        out |= _rect(x, y, t, t)
    return out


def _ascii(rows):
    return {(x, y) for y, r in enumerate(rows) for x, c in enumerate(r) if c == "#"}


SPKR = _rect(0, 5, 3, 4) | _tri(3, 2, 5, 10, "l")
PX = {
    "play": _tri(2, 0, 11, 14, "r"),
    "pause": _rect(2, 1, 4, 12) | _rect(8, 1, 4, 12),
    "stop": _rect(2, 2, 10, 10),
    "prev": _rect(1, 1, 2, 12) | _tri(3, 1, 10, 12, "l"),
    "next": _rect(11, 1, 2, 12) | _tri(1, 1, 10, 12, "r"),
    "shuffle": _line(0, 3, 3, 3) | _line(3, 3, 8, 10) | _line(8, 10, 10, 10) | _line(0, 10, 3, 10) | _line(3, 10, 8, 3) | _line(8, 3, 10, 3) | _tri(10, 1, 3, 5, "r") | _tri(10, 8, 3, 5, "r"),
    "repeat": _rect(1, 3, 9, 2) | _rect(1, 3, 2, 5) | _rect(4, 9, 9, 2) | _rect(11, 6, 2, 5) | _tri(10, 1, 3, 6, "r") | _tri(1, 7, 3, 6, "l"),
    "repeat-one": None,
    "volume": SPKR | _rect(10, 5, 1, 4) | _rect(12, 3, 1, 8),
    "volume-low": SPKR | _rect(10, 5, 1, 4),
    "mute": SPKR | _line(9, 4, 12, 9, 2) | _line(12, 4, 9, 9, 2),
    "heart": _ascii(["..............", ".####....####.", "######..######", "##############", "##############", "##############", ".############.", "..##########..", "...########...", "....######....", ".....####.....", "......##......", "..............", ".............."]),
    "search": _ring(5, 5, 5, 2) | _line(8, 8, 12, 12, 3),
    "library": _rect(0, 2, 3, 10) | _rect(4, 2, 3, 10) | _line(8, 3, 11, 12, 3),
    "now": _ring(7, 7, 6, 2) | _disc(7, 7, 2),
    "lists": _rect(0, 2, 8, 2) | _rect(0, 6, 8, 2) | _rect(0, 10, 5, 2) | _rect(11, 3, 2, 7) | _disc(10, 10, 2),
    "discover": _ring(7, 7, 6, 2) | _line(4, 9, 9, 4, 1) | _rect(6, 6, 2, 2),
    "settings": (_disc(7, 7, 5) - _disc(7, 7, 2)) | _rect(6, 0, 2, 3) | _rect(6, 11, 2, 3) | _rect(0, 6, 3, 2) | _rect(11, 6, 3, 2) | _rect(2, 2, 2, 2) | _rect(10, 2, 2, 2) | _rect(2, 10, 2, 2) | _rect(10, 10, 2, 2),
    "add": _rect(5, 1, 4, 12) | _rect(1, 5, 12, 4),
    "playlist-add": _rect(0, 2, 9, 2) | _rect(0, 6, 6, 2) | _rect(0, 10, 9, 2) | _rect(10, 5, 2, 8) | _rect(7, 8, 8, 2),
    "close": _line(1, 1, 10, 10, 3) | _line(10, 1, 1, 10, 3),
    "back": _line(8, 0, 2, 6, 3) | _line(2, 6, 8, 12, 3),
    "forward": _line(3, 0, 9, 6, 3) | _line(9, 6, 3, 12, 3),
    "down": _line(0, 3, 6, 9, 3) | _line(6, 9, 12, 3, 3),
    "sort-up": _tri(2, 3, 10, 8, "u"),
    "sort-down": _tri(2, 3, 10, 8, "d"),
    "more": _rect(0, 5, 3, 3) | _rect(5, 5, 3, 3) | _rect(10, 5, 3, 3),
    "refresh": (_ring(7, 7, 6, 2) - _rect(7, 0, 7, 5)) | _tri(8, 0, 6, 6, "r"),
    "music": _disc(4, 10, 3) | _rect(6, 1, 2, 10) | _rect(6, 1, 7, 3),
    "lyrics": (_ring(7, 6, 5, 2) - _rect(0, 0, 14, 6)) | _rect(5, 0, 4, 8) | _rect(6, 10, 2, 3) | _rect(4, 12, 6, 2),
    "history": _ring(7, 7, 6, 2) | _rect(7, 3, 1, 5) | _rect(7, 7, 3, 1),
    "user": _disc(7, 4, 3) | (_disc(7, 14, 6) & _rect(0, 8, 14, 6)),
    "list": _rect(0, 2, 14, 2) | _rect(0, 6, 14, 2) | _rect(0, 10, 14, 2),
    "check": _line(0, 7, 4, 11, 3) | _line(4, 11, 11, 2, 3),
    "equalizer": _rect(1, 7, 3, 6) | _rect(5, 2, 3, 11) | _rect(9, 5, 3, 8),
    "no-image": (_rect(0, 1, 14, 12) - _rect(2, 3, 10, 8)) | _line(2, 11, 11, 2, 2),
    "folder": _rect(0, 2, 6, 3) | _rect(0, 4, 14, 8),
}


PX["repeat-one"] = PX["repeat"] | _rect(6, 5, 2, 5) | _rect(5, 5, 1, 1)


def pixel(name, hue):
    cells = {(x + 1, y + 1) for x, y in PX[name]}
    outline = {(x + dx, y + dy) for x, y in cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))} - cells
    fill, light = hsl(hue, 0.7, 0.55), hsl(hue, 0.75, 0.78)
    rects = []
    for (x, y), colour in sorted([(c, "#1b1b24") for c in outline] + [(c, light if (c[0], c[1] - 1) not in cells else fill) for c in cells], key=lambda t: (t[0][1], t[0][0])):
        if 0 <= x < 16 and 0 <= y < 16:
            rects.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="{colour}"/>')
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" shape-rendering="crispEdges">{"".join(rects)}</svg>\n'


def main():
    for s in ("line", "aero", "pixel"):
        os.makedirs(f"{HERE}/{s}", exist_ok=True)
    for name, (src, hue, glyph) in G.items():
        # the line set is copied from the original Lucide files (kept in ui/ until the move)
        lucide = f"{HERE}/ui/{src}.svg"
        if os.path.exists(lucide):
            shutil.copy(lucide, f"{HERE}/line/{name}.svg")
        open(f"{HERE}/aero/{name}.svg", "w").write(aero(name, hue, glyph))
        open(f"{HERE}/pixel/{name}.svg", "w").write(pixel(name, hue))
    man = {"names": list(G), "sets": {"line": {"label": "Line", "mono": True}, "aero": {"label": "Aero", "mono": False}, "pixel": {"label": "Pixel", "mono": False}}}
    json.dump(man, open(f"{HERE}/manifest.json", "w"), indent=1)


main()
