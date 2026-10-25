#!/usr/bin/env python3
"""Draws VOID Lodestone's logo: a flat-shaded, low-poly lodestone in a bevelled gold frame (the old-school look), with
the VOID spiral arms and spark carved into it. Writes art/logo.svg; scripts/icons.sh renders the PNG sizes.

The arms and spark follow void-ui's VoidEmblem (108-unit grid, THETA_MAX = 1.7 pi), scaled onto the 512 canvas."""
import math
import sys
from pathlib import Path

C = 256.0
LIGHT = (-0.55, -0.83)  # light from the top left, like the old game's interfaces


def shade(angle: float) -> float:
    """0..1 brightness of a face whose outward normal points at `angle`."""
    nx, ny = math.cos(angle), math.sin(angle)
    return max(0.0, min(1.0, 0.5 + 0.5 * (nx * LIGHT[0] + ny * LIGHT[1])))


def lerp_hex(a: str, b: str, t: float) -> str:
    pa = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return '#' + ''.join(f'{round(x + (y - x) * t):02x}' for x, y in zip(pa, pb))


def ramp(stops: list[str], t: float) -> str:
    t = max(0.0, min(1.0, t)) * (len(stops) - 1)
    i = min(int(t), len(stops) - 2)
    return lerp_hex(stops[i], stops[i + 1], t - i)


def poly(points, **attrs) -> str:
    pts = ' '.join(f'{x:.1f},{y:.1f}' for x, y in points)
    extra = ' '.join(f'{k.replace("_", "-")}="{v}"' for k, v in attrs.items())
    return f'<polygon points="{pts}" {extra}/>'


def ring(n: int, r: float, rot: float):
    return [(C + r * math.cos(rot + 2 * math.pi * i / n), C + r * math.sin(rot + 2 * math.pi * i / n)) for i in range(n)]


GOLD = ['#3d2a0c', '#7a5518', '#b8862b', '#e8be5c', '#fbe7a6']
STONE = ['#100c20', '#1f1838', '#342a5c', '#4d3f82', '#6e5cae']


def frame() -> list[str]:
    """Octagonal bevel: outer lip, flat band, inner lip, each face flat-shaded."""
    out = []
    rot = math.pi / 8
    layers = [(206, 194, 0), (194, 176, 1), (176, 166, 2)]
    for r_out, r_in, kind in layers:
        a, b = ring(8, r_out, rot), ring(8, r_in, rot)
        for i in range(8):
            j = (i + 1) % 8
            mid = rot + 2 * math.pi * (i + 0.5) / 8
            s = shade(mid)
            if kind == 1:  # the flat band reads as lit metal, less contrast
                s = 0.45 + 0.35 * s
            elif kind == 2:  # inner lip faces inwards: light falls the other way
                s = 1 - s
            out.append(poly([a[i], a[j], b[j], b[i]], fill=ramp(GOLD, s)))
    # rivets on the band's corners
    for x, y in ring(8, 185, rot):
        out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="6.5" fill="#4a3310"/>')
        out.append(f'<circle cx="{x - 1.4:.1f}" cy="{y - 1.6:.1f}" r="4.6" fill="{GOLD[3]}"/>')
        out.append(f'<circle cx="{x - 2.4:.1f}" cy="{y - 2.6:.1f}" r="1.8" fill="{GOLD[4]}"/>')
    return out


def stone() -> list[str]:
    """A 16-sided dome: an outer bevel ring and an inner fan, flat-shaded per facet."""
    out = []
    n = 16
    rot = math.pi / 16
    outer, mid = ring(n, 166, rot), ring(n, 128, rot)
    for i in range(n):
        j = (i + 1) % n
        ang = rot + 2 * math.pi * (i + 0.5) / n
        out.append(poly([outer[i], outer[j], mid[j], mid[i]], fill=ramp(STONE, 0.15 + 0.7 * shade(ang))))
    top = (C - 10, C - 14)  # the dome's peak sits a little towards the light
    for i in range(n):
        j = (i + 1) % n
        ang = rot + 2 * math.pi * (i + 0.5) / n
        out.append(poly([mid[i], mid[j], top], fill=ramp(STONE, 0.22 + 0.5 * shade(ang) + (0.07 if i % 2 else 0))))
    # carved rune marks on the rim at the four compass points
    for q in range(4):
        a = -math.pi / 2 + q * math.pi / 2
        ux, uy = math.cos(a), math.sin(a)
        px, py = -uy, ux
        cx, cy = C + 147 * ux, C + 147 * uy
        tri = [(cx + 9 * ux, cy + 9 * uy), (cx - 7 * ux + 8 * px, cy - 7 * uy + 8 * py), (cx - 7 * ux - 8 * px, cy - 7 * uy - 8 * py)]
        out.append(poly([(x + 1.5, y + 2) for x, y in tri], fill='#07050f', opacity='0.8'))
        out.append(poly(tri, fill='none', stroke='#d9b25a', stroke_width='3', stroke_linejoin='round', opacity='0.9'))
    return out


# ---- VOID emblem, after void-ui's VoidEmblem ----
THETA_MAX = 1.7 * math.pi
K = 3.55  # 108-unit emblem grid -> logo pixels


def arm(phase: float, t0: float, t1: float, width: float, n: int = 40) -> str:
    pts = []
    for i in range(n + 1):
        t = t0 + (t1 - t0) * i / n
        th = t * THETA_MAX
        r = (2.5 + 36.0 * t ** 1.25) * K
        pts.append((C + r * math.cos(th + phase), C + r * math.sin(th + phase), t))
    left, right = [], []
    for i, (x, y, t) in enumerate(pts):
        a, b = pts[max(i - 1, 0)], pts[min(i + 1, n)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        d = math.hypot(dx, dy) or 1.0
        w = (0.3 + 2.5 * math.sin(math.pi * min(t, 0.999) ** 0.55)) * width * 0.9 / 2 * K
        left.append((x - dy / d * w, y + dx / d * w))
        right.append((x + dy / d * w, y - dx / d * w))
    p = left + right[::-1]
    return 'M' + ' L'.join(f'{x:.1f},{y:.1f}' for x, y in p) + ' Z'


def spark(r: float) -> str:
    k = r / 9.5

    def p(dx, dy):
        return f'{C + dx * k:.1f},{C + dy * k:.1f}'
    segs = [((0.9, -3.4), (3.4, -0.9), (9.5, 0)), ((3.4, 0.9), (0.9, 3.4), (0, 9.5)),
            ((-0.9, 3.4), (-3.4, 0.9), (-9.5, 0)), ((-3.4, -0.9), (-0.9, -3.4), (0, -9.5))]
    d = f'M{p(0, -9.5)}'
    for c1, c2, e in segs:
        d += f' C{p(*c1)} {p(*c2)} {p(*e)}'
    return d + ' Z'


def emblem() -> list[str]:
    out = []
    arms = [(-math.pi / 2, '#22e3ff'), (math.pi / 2, '#c9a4ff')]
    # the carved channel: a dark groove the light glows out of
    for phase, _ in arms:
        out.append(f'<path d="{arm(phase, 0, 1, 2.0)}" fill="#07050f" transform="translate(3 4)" opacity="0.85"/>')
    for phase, color in arms:
        out.append(f'<path d="{arm(phase, 0, 1, 4.2)}" fill="{color}" opacity="0.16" filter="url(#soft)"/>')
        out.append(f'<path d="{arm(phase, 0, 1, 2.4)}" fill="{color}" opacity="0.35" filter="url(#soft)"/>')
        segs = 14
        for k in range(segs):
            t0 = k / segs
            t1 = min(1.0, (k + 1.35) / segs)
            tm = (t0 + t1) / 2
            c = lerp_hex('#ffffff', color, min(1.0, tm / 0.22))
            a = 1.0 if tm < 0.55 else 1.0 - 0.5 * (tm - 0.55) / 0.45
            out.append(f'<path d="{arm(phase, t0, t1, 1.0, 8)}" fill="{c}" opacity="{a:.2f}"/>')
    out.append(f'<circle cx="{C}" cy="{C}" r="44" fill="url(#core)"/>')
    out.append(f'<path d="{spark(30)}" fill="url(#sparkfill)"/>')
    return out


def build() -> str:
    parts = [
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">',
        '<title>VOID Lodestone</title>',
        '<defs>',
        '<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#171230"/>'
        '<stop offset="1" stop-color="#090713"/></linearGradient>',
        '<radialGradient id="aura" cx="0.5" cy="0.5" r="0.5"><stop offset="0.55" stop-color="#a970ff" stop-opacity="0.35"/>'
        '<stop offset="1" stop-color="#a970ff" stop-opacity="0"/></radialGradient>',
        '<radialGradient id="core" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#22e3ff" stop-opacity="0.75"/>'
        '<stop offset="1" stop-color="#22e3ff" stop-opacity="0"/></radialGradient>',
        '<radialGradient id="sparkfill" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#ffffff"/>'
        '<stop offset="1" stop-color="#b6f5ff"/></radialGradient>',
        '<filter id="soft" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="5"/></filter>',
        '</defs>',
        '<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>',
        '<rect x="16" y="16" width="480" height="480" rx="112" fill="none" stroke="#2a2346" stroke-width="6"/>',
        f'<circle cx="{C}" cy="{C}" r="236" fill="url(#aura)"/>',
        # drop shadow under the frame
        poly([(x + 6, y + 9) for x, y in ring(8, 208, math.pi / 8)], fill='#000000', opacity='0.45'),
    ]
    parts += frame()
    parts += stone()
    parts += emblem()
    parts.append('</svg>')
    return '\n'.join(parts) + '\n'


if __name__ == '__main__':
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name('logo.svg')
    out.write_text(build())
    print(out)
