"""DRIFT final: sprite builder (24x18 panel sprite + 16x16 icon), sprite JSON, SVG export, reference sheet.

Colour keys map 1:1 to Nightgrid tokens so the sprite follows the active theme.
"""
import json, os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))

# key -> (token, fallback hex)
KEYS = {
    "M": ("--mint", "#BDF5EC"),          # hull hairline, antenna, fins
    "m": ("mint-35", "#4F6A68"),       # dim mint: empty stepper seg (mint @ 35% on bg-2)
    "D": ("--bg-2", "#141821"),          # hull fill
    "B": ("--bg-0", "#07080B"),          # visor
    "L": ("--line", "#1F2633"),          # readout bars
    "C": ("--cyan", "#00E5FF"),          # eyes
    "c": ("cyan-35", "#0A5560"),      # dim eyes (sleep)
    "W": ("--text-0", "#E6EDF3"),        # eye glint
    "K": ("--magenta", "#FF2A6D"),       # thruster, active stepper seg
    "k": ("magenta-45", "#7A1736"),   # thruster tail
    "Y": ("--accent", "#F3E600"),        # status LED
    "G": ("--ok", "#2BFF88"),
    "R": ("--danger", "#FF003C"),
    "O": ("--warn", "#FF9F1C"),
}
HEX = {k: v[1] for k, v in KEYS.items()}
# Derived colours (not separate tokens): mixed from real tokens so themes still recolour them.
DERIVED = {
    "m": "color-mix(in srgb, var(--mint) 35%, var(--bg-2))",
    "c": "color-mix(in srgb, var(--cyan) 35%, var(--bg-0))",
    "k": "color-mix(in srgb, var(--magenta) 45%, var(--bg-0))",
}

def css_color(k):
    return DERIVED.get(k) or "var(%s, %s)" % KEYS[k]
GLOW = set("CKYGRO")
W, H = 24, 18

def blank(w=W, h=H):
    return [["."] * w for _ in range(h)]

def put(g, x, y, ch):
    if 0 <= y < len(g) and 0 <= x < len(g[0]):
        g[y][x] = ch

def hline(g, x0, x1, y, ch):
    for x in range(x0, x1 + 1):
        put(g, x, y, ch)

def vline(g, x, y0, y1, ch):
    for y in range(y0, y1 + 1):
        put(g, x, y, ch)

def hull(g, x0, y0, x1, y1, cut=3, fill="D", line="M"):
    """Rectangle with chamfered top-right and bottom-left corners (same cut as the app icon and panels)."""
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            # outside the chamfers?
            if (x1 - x) + (y - y0) < cut - 0:  # top-right triangle
                if (x1 - x) + (y - y0) < cut - 1:
                    continue
            if (x - x0) + (y1 - y) < cut - 1:  # bottom-left triangle
                continue
            edge = x in (x0, x1) or y in (y0, y1) or (x1 - x) + (y - y0) == cut - 1 or (x - x0) + (y1 - y) == cut - 1
            put(g, x, y, line if edge else fill)

def eye(g, x, y, kind, col="C"):
    """3x3 eye at (x,y). kinds: open, glint, blink, line, up, happy, x, dot, half"""
    pats = {
        "open": ["WCC", "CCC", "CCC"],
        "blink": ["...", "...", "CCC"],
        "line": ["...", "CCC", "..."],
        "half": ["...", "CCC", "CCC"],
        "up": ["WCC", "CCC", "..."],
        "happy": [".C.", "C.C", "..."],
        "x": ["C.C", ".C.", "C.C"],
        "squint": ["...", "CCC", "..."],
    }
    for dy, row in enumerate(pats[kind]):
        for dx, ch in enumerate(row):
            if ch != ".":
                put(g, x + dx, y + dy, col if ch == "C" else ch)

def drift(state="idle"):
    g = blank()
    s = state
    led = {"success": "G", "error": "R", "sleep": "L", "think": "Y"}.get(s, "Y")
    ant = led if s in ("success", "error") else "Y" if s != "sleep" else "m"
    # antenna (left, with LED) + short stub (right)
    put(g, 5, 0, ant); vline(g, 5, 1, 2, "M")
    vline(g, 18, 1, 2, "M")
    # hull
    hull(g, 2, 3, 21, 14)
    # fins
    for fx in (1, 22):
        vline(g, fx, 8, 11, "M")
    # forehead stepper (mirrors the app's top stepper): 3 segments
    seg = {"idle": "KMm", "listen": "KMm", "point": "KMm", "talk": "KMm", "think": "KKm",
           "success": "MMM", "error": "RRR", "sleep": "mmm", "confused": "KmY"}.get(s, "KMm")
    for i, ch in enumerate(seg):
        hline(g, 5 + i * 5, 8 + i * 5, 5, ch)
    # visor
    for y in range(8, 13):
        hline(g, 4, 19, y, "B")
    ex = (6, 15)
    if s == "point":
        ex = (8, 17)
    if s in ("idle", "listen", "point"):
        eye(g, ex[0], 9, "open"); eye(g, ex[1], 9, "open")
    elif s == "blink":
        eye(g, 6, 9, "blink"); eye(g, 15, 9, "blink")
    elif s == "think":
        hline(g, 7, 16, 10, "C")
    elif s == "talk":
        eye(g, 6, 9, "up"); eye(g, 15, 9, "up")
        for x, on in zip(range(9, 15), "C.CC.C"):
            if on == "C":
                put(g, x, 11, "C")
        hline(g, 10, 13, 12, "C")
    elif s == "success":
        eye(g, 6, 9, "happy"); eye(g, 15, 9, "happy")
    elif s == "error":
        eye(g, 6, 9, "x", "R"); eye(g, 15, 9, "x", "R")
    elif s == "sleep":
        eye(g, 6, 9, "line", "c"); eye(g, 15, 9, "line", "c")
    elif s == "confused":
        eye(g, 6, 9, "open"); eye(g, 15, 9, "squint")
    if s == "listen":
        hline(g, 11, 12, 12, "C")
    # readout + LED
    hline(g, 14, 16, 13, "L"); put(g, 18, 13, led)
    # thruster
    if s not in ("sleep",):
        if s in ("think", "error"):
            hline(g, 11, 12, 15, "K" if s == "think" else "k")
        else:
            hline(g, 10, 13, 15, "K"); hline(g, 11, 12, 16, "k")
    return ["".join(r) for r in g]

def icon16(state="idle"):
    """16x16 status-bar icon (V5 pod proportions, same colour roles)."""
    g = blank(16, 16)
    put(g, 4, 0, "Y"); put(g, 4, 1, "M")
    hull(g, 1, 2, 14, 12, cut=2)
    hline(g, 3, 5, 3, "K"); hline(g, 7, 9, 3, "M"); hline(g, 11, 12, 3, "m")
    for y in range(5, 10):
        hline(g, 3, 12, y, "B")
    if state == "think":
        hline(g, 4, 11, 7, "C")
    else:
        for x in (4, 9):
            for dx in range(3):
                for dy in range(3):
                    put(g, x + dx, 6 + dy, "C")
            put(g, x, 6, "W")
    put(g, 12, 11, "Y" if state != "error" else "R")
    hline(g, 6, 9, 13, "K"); hline(g, 7, 8, 14, "k")
    return ["".join(r) for r in g]

STATES = ["idle", "blink", "think", "talk", "point"]
MOODS = ["listen", "success", "error", "confused", "sleep"]

# ------------------------------------------------------------------ SVG
def rects(rows, px, ox=0, oy=0, glow=True, use_vars=False):
    groups = {}
    for y, row in enumerate(rows):
        x = 0
        while x < len(row):
            ch = row[x]
            if ch == ".":
                x += 1; continue
            run = 1
            while x + run < len(row) and row[x + run] == ch:
                run += 1
            fill = 'style="fill:%s"' % css_color(ch) if use_vars else 'fill="%s"' % HEX[ch]
            groups.setdefault(ch, []).append('<rect x="%g" y="%g" width="%g" height="%g" %s/>' % (ox + x * px, oy + y * px, run * px, px, fill))
            x += run
    out = []
    for ch, r in groups.items():
        out.append('<g filter="url(#d-glow-%s)">%s</g>' % (ch, "".join(r)) if glow and ch in GLOW else "".join(r))
    return "".join(out)

def glow_defs(std=2.2):
    d = "".join('<filter id="d-glow-%s" x="-50%%" y="-50%%" width="200%%" height="200%%"><feGaussianBlur stdDeviation="%g" result="b"/>'
                '<feFlood flood-color="%s" flood-opacity=".55"/><feComposite in2="b" operator="in"/><feMerge><feMergeNode/>'
                '<feMergeNode in="SourceGraphic"/></feMerge></filter>' % (k, std, HEX[k]) for k in GLOW)
    return "<defs>%s</defs>" % d

def svg(w, h, body, attrs=""):
    return '<svg xmlns="http://www.w3.org/2000/svg" width="%g" height="%g" viewBox="0 0 %g %g" shape-rendering="crispEdges" %s>%s%s</svg>' % (w, h, w, h, attrs, glow_defs(), body)

def sprite_svg_file(rows):
    """Standalone sprite: 1 unit per pixel, colours via CSS vars with fallbacks, no glow (the app adds glow via CSS)."""
    w, h = len(rows[0]), len(rows)
    return '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %d %d" width="%d" height="%d" shape-rendering="crispEdges">%s</svg>\n' % (
        w, h, w * 4, h * 4, rects(rows, 1, glow=False, use_vars=True))

# ------------------------------------------------------------------ reference sheet
def center(rows, bw, bh, px, glow=True, extra=""):
    gw, gh = len(rows[0]) * px, len(rows) * px
    return svg(bw, bh, rects(rows, px, (bw - gw) / 2, (bh - gh) / 2, glow) + extra)

def spot_button(x, y, w, h, label):
    ch = 6
    s = '<polygon points="%g,%g %g,%g %g,%g %g,%g %g,%g" fill="#0D0F14" stroke="#FF2A6D" stroke-width="1" shape-rendering="geometricPrecision"/>' % (
        x, y, x + w - ch, y, x + w, y + ch, x + w, y + h, x, y + h)
    s += '<text x="%g" y="%g" font-family="JetBrains Mono" font-size="11" letter-spacing="1.4" fill="#E6EDF3" text-anchor="middle">%s</text>' % (x + w / 2, y + h / 2 + 4, label)
    g, L = 5, 9
    for cx, cy, sx, sy in ((x - g, y - g, 1, 1), (x + w + g, y - g, -1, 1), (x - g, y + h + g, 1, -1), (x + w + g, y + h + g, -1, -1)):
        s += '<path d="M%g %g L%g %g L%g %g" stroke="#FF2A6D" stroke-width="2" fill="none" filter="url(#d-glow-K)"/>' % (cx, cy + sy * L, cx, cy, cx + sx * L, cy)
    return s

def construction(px=16):
    rows = drift("idle")
    w, h = len(rows[0]) * px, len(rows) * px
    body = rects(rows, px, 0, 0, glow=False)
    grid = "".join('<line x1="%d" y1="0" x2="%d" y2="%d" stroke="#E6EDF3" stroke-opacity=".08"/>' % (x * px, x * px, h) for x in range(len(rows[0]) + 1))
    grid += "".join('<line x1="0" y1="%d" x2="%d" y2="%d" stroke="#E6EDF3" stroke-opacity=".08"/>' % (y * px, w, y * px) for y in range(len(rows) + 1))
    return body + grid, w, h

CSS = """
@import url('https://fonts.googleapis.com/css2?family=Rajdhani:wght@500;600;700&display=block');
*{box-sizing:border-box;margin:0;padding:0}
body{background:#07080B;color:#E6EDF3;font-family:'JetBrains Mono',monospace;width:1400px;padding:36px 40px 40px;
 background-image:radial-gradient(#141821 1px,transparent 1px);background-size:16px 16px}
.hdr{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #1F2633;padding-bottom:14px;margin-bottom:20px}
.hdr h1{font-family:Rajdhani;font-weight:700;font-size:60px;letter-spacing:.06em;line-height:.9}
.hdr h1 em{font-style:normal;color:#FF2A6D}
.hdr .sub{font-size:12px;letter-spacing:.14em;color:#8B98A9;margin-top:8px}
.hdr .meta{font-size:11px;color:#8B98A9;letter-spacing:.14em;text-align:right;line-height:1.7}
.tag{display:inline-block;font-size:11px;letter-spacing:.14em;color:#00E5FF;border:1px solid #00E5FF55;padding:3px 8px;margin-right:6px}
.row{display:flex;gap:16px;margin-bottom:16px}
.box{border:1px solid #1F2633;background:#0D0F14E6;padding:14px 16px;clip-path:polygon(0 0,calc(100% - 10px) 0,100% 10px,100% 100%,10px 100%,0 calc(100% - 10px))}
.box>.t{font-size:11px;letter-spacing:.14em;color:#8B98A9;margin-bottom:10px;text-transform:uppercase}
.cells{display:flex;gap:10px}
.cell{flex:1;background:#07080B;border:1px solid #1F2633;padding:8px}
.cell .l{font-size:10.5px;letter-spacing:.14em;color:#FF2A6D}
.cell .n{font-size:10px;line-height:1.45;color:#8B98A9;margin-top:4px;min-height:42px}
.cell svg{display:block;margin:4px auto 0}
.sw{display:grid;grid-template-columns:1fr 1fr;gap:7px 16px}
.sw div{display:flex;align-items:center;gap:8px;font-size:10.5px}
.sw i{width:20px;height:20px;border:1px solid #1F2633;flex:none}
.sw b{font-weight:600;min-width:98px}
.sw span{color:#8B98A9}
.lad{display:flex;gap:24px;align-items:flex-end}
.lad div{display:flex;flex-direction:column;align-items:center;gap:6px;font-size:10px;color:#8B98A9}
ul.n{list-style:none;font-size:11px;line-height:1.75;color:#C9D3DE}
ul.n li::before{content:"// ";color:#00E5FF}
.anno{font-size:10.5px;line-height:1.9;color:#8B98A9}
.anno b{color:#E6EDF3;font-weight:600}
"""

STATE_NOTES = {
    "idle": "Hover bob 2px / 2.4s. Blink every 4–7s (BLINK frame 120ms).",
    "blink": "Single frame, 120ms. Also the transition frame between moods.",
    "think": "Eyes merge into a scan bar sweeping L↔R; stepper fills seg by seg; LED pulses.",
    "talk": "Mouth waveform frames swap per streamed chunk (~90ms); eyes look up.",
    "point": "Eyes shift toward target; magenta brackets lock around the control.",
    "listen": "User is typing a prompt. Small mouth, steady stepper.",
    "success": "^ ^ eyes, stepper all mint, LED + antenna --ok. Holds 1.5s then IDLE.",
    "error": "x x eyes in --danger, stepper red, thruster dims. Stays until dismissed.",
    "confused": "One eye squints + yellow ?: the model needs more context.",
    "sleep": "Helper off / model unloaded. No thruster, dim lids, no glow.",
}

def sheet():
    cons, cw, chh = construction(15)
    hero = center(drift("idle"), 400, 360, 14)
    st = "".join('<div class="cell"><div class="l">%s</div>%s<div class="n">%s</div></div>' % (
        s.upper(), center(drift(s), 200, 150, 7, extra=('<g>' + "".join('<rect x="%d" y="18" width="5" height="5" fill="#F3E600" opacity="%s"/>' % (160 + i * 9, o) for i, o in enumerate((1, 1, .35))) + '</g>') if s == "think" else ""), STATE_NOTES[s]) for s in STATES)
    # pointing demo, wide
    prow = drift("point")
    point_demo = svg(360, 150, rects(prow, 6, 10, 21) + '<line x1="160" y1="75" x2="196" y2="75" stroke="#FF2A6D" stroke-width="1.5" stroke-dasharray="4 3" filter="url(#d-glow-K)" shape-rendering="geometricPrecision"/>' + spot_button(206, 60, 120, 30, "RESTART"))
    mo = "".join('<div class="cell"><div class="l">%s</div>%s<div class="n">%s</div></div>' % (
        s.upper(), center(drift(s), 200, 150, 7, extra=(
            '<text x="172" y="36" font-family="JetBrains Mono" font-weight="700" font-size="18" fill="#F3E600">?</text>' if s == "confused" else
            '<text x="166" y="38" font-family="JetBrains Mono" font-size="14" fill="#8B98A9">z</text><text x="177" y="26" font-family="JetBrains Mono" font-size="11" fill="#8B98A9">z</text>' if s == "sleep" else ""),
            glow=s != "sleep"), STATE_NOTES[s]) for s in MOODS)
    lad = ""
    for size in (96, 72, 48, 32, 24):
        rows = drift("idle")
        px = size / 24
        lad += '<div>%s<span>%dpx</span></div>' % (svg(size, size * 18 / 24, rects(rows, px, 0, 0, glow=size >= 48)), size)
    for size in (16, 16):
        pass
    ic = icon16()
    lad += '<div style="margin-left:18px;padding-left:24px;border-left:1px solid #1F2633">%s<span>icon 16 (×1)</span></div>' % svg(16, 16, rects(ic, 1, glow=False))
    lad += '<div>%s<span>icon 16 (×2)</span></div>' % svg(32, 32, rects(ic, 2, glow=False))
    lad += '<div>%s<span>icon 16 (×4)</span></div>' % svg(64, 64, rects(ic, 4, glow=False))
    pal = "".join('<div><i style="background:%s"></i><b>%s</b><span>%s</span></div>' % (HEX[k], KEYS[k][0], u) for k, u in (
        ("M", "hull hairline, antenna, fins"), ("D", "hull fill"), ("B", "visor"), ("C", "eyes, waveform"),
        ("K", "thruster, stepper seg 1, spotlight"), ("Y", "status LED, thinking"), ("m", "empty seg (mint 35% mix)"),
        ("W", "eye glint"), ("G", "success"), ("R", "error")))
    return """<!doctype html><html><head><meta charset="utf-8"><style>%s</style></head><body>
<div class="hdr"><div><div style="margin-bottom:10px"><span class="tag">REFERENCE</span><span class="tag">24×18 PIXEL SPRITE</span><span class="tag">16×16 ICON</span></div>
<h1><em>DRIFT</em></h1><div class="sub">NIGHTGRID LLM HELPER // HUD MAINTENANCE DRONE</div></div>
<div class="meta">NIGHTGRID // MASCOT REFERENCE<br>REV 1.0 // 2026-11-14<br>ORIGINAL DESIGN</div></div>
<div class="row">
 <div class="box" style="width:440px"><div class="t">Hero // idle, effects on</div>%s</div>
 <div class="box" style="flex:1"><div class="t">Construction // 24×18 grid</div><div style="display:flex;gap:18px;align-items:center">
  <div>%s</div>
  <div class="anno"><b>Hull</b> --bg-2 fill, 1px --mint hairline, chamfered top-right + bottom-left (3px), same cut as the app icon and panels.<br>
  <b>Stepper</b> 3 × 4px segments on the forehead, the app's top stepper in miniature: magenta / mint / empty.<br>
  <b>Visor</b> --bg-0, two 3×3 --cyan eyes with a --text-0 glint.<br>
  <b>Readout</b> --line bars + --accent status LED.<br>
  <b>Thruster</b> --magenta, the only part that loops.<br>
  <b>Antenna</b> left, LED tip mirrors status; short stub right.<br>
  <b>Fins</b> 1px mint, kept thin so mint stays an outline colour, not a fill.</div></div></div>
</div>
<div class="row"><div class="box" style="flex:1"><div class="t">Core states</div><div class="cells">%s</div></div></div>
<div class="row"><div class="box" style="flex:1"><div class="t">Moods</div><div class="cells">%s</div></div></div>
<div class="row">
 <div class="box" style="flex:1.1"><div class="t">Guiding // spotlight a control</div>%s
  <ul class="n" style="margin-top:6px"><li>Brackets are drawn on the target, not inside the sprite.</li><li>Reduce effects: brackets appear instantly, no beam.</li></ul></div>
 <div class="box" style="flex:1"><div class="t">Palette // tokens (themes recolour it)</div><div class="sw">%s</div></div>
</div>
<div class="row">
 <div class="box" style="flex:1.1"><div class="t">Sizes // integer scales only</div><div class="lad">%s</div></div>
 <div class="box" style="flex:1"><div class="t">Reduce effects</div><ul class="n">
  <li>No bob, no glow, no beam, no thruster flicker.</li><li>State changes stay: instant frame swaps.</li>
  <li>THINK shows a static bar; TALK a static mouth.</li><li>Glow is also off below 48px.</li></ul></div>
</div>
</body></html>""" % (CSS, hero, svg(cw, chh, cons), st, mo, point_demo, pal, lad)

if __name__ == "__main__":
    os.makedirs(os.path.join(OUT, "sprites"), exist_ok=True)
    data = {"name": "DRIFT", "grid": [W, H], "icon_grid": [16, 16],
            "palette": {k: {"css": css_color(k), "fallback": v[1]} for k, v in KEYS.items()},
            "glow_keys": sorted(GLOW), "frames": {}, "icon": {"idle": icon16("idle"), "think": icon16("think"), "error": icon16("error")}}
    for s in STATES + MOODS:
        data["frames"][s] = drift(s)
        open(os.path.join(OUT, "sprites", "drift-%s.svg" % s), "w").write(sprite_svg_file(drift(s)))
    for s in ("idle", "think", "error"):
        open(os.path.join(OUT, "sprites", "drift-icon16-%s.svg" % s), "w").write(sprite_svg_file(icon16(s)))
    json.dump(data, open(os.path.join(OUT, "drift-sprites.json"), "w"), indent=1)
    open(os.path.join(OUT, "drift-reference.html"), "w").write(sheet())
    for s in STATES + MOODS:
        assert all(len(r) == W for r in drift(s)) and len(drift(s)) == H
    print("ok")
