#!/usr/bin/env python3
"""Draws the Library's figures (docs/kb/figures/*.svg): guide briefing banners
and diagrams, in Nightgrid's own quest-log style (dark intel cards, amber
headers, cyan line art, magenta highlights). Plain SVG with no scripts, fonts
or outside references, so the files stay small and safe to bundle.

    python3 scripts/kb_figures.py            # writes every figure
    python3 scripts/kb_figures.py guide-nas  # only the ones whose name starts so

Add a figure by writing a function below and listing it in FIGURES; reference
it from an article as `![caption](fig:name)` on a line of its own.
"""

from __future__ import annotations

import sys
from pathlib import Path
from xml.sax.saxutils import escape

OUT = Path(__file__).resolve().parent.parent / "docs" / "kb" / "figures"

# Palette (matches ui/src/styles/tokens.css, dark theme).
BG = "#07080B"
PANEL = "#0D0F14"
LINE = "#1F2633"
GRID = "#11161F"
TEXT = "#E6EDF3"
DIM = "#8B98A9"
CYAN = "#00E5FF"
MAGENTA = "#FF2A6D"
AMBER = "#FF9F1C"
MINT = "#BDF5EC"
MONO = "'JetBrains Mono',Menlo,Consolas,monospace"
SANS = "Rajdhani,'Segoe UI',system-ui,sans-serif"


def t(x, y, s, size=14, fill=TEXT, anchor="start", weight=400, family=MONO, spacing=0, opacity=1.0):
    extra = f' letter-spacing="{spacing}"' if spacing else ""
    op = f' opacity="{opacity}"' if opacity != 1 else ""
    return (
        f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" fill="{fill}" '
        f'text-anchor="{anchor}" font-weight="{weight}"{extra}{op}>{escape(s)}</text>'
    )


def chamfer(x, y, w, h, c=10):
    return f"M{x},{y} H{x + w - c} L{x + w},{y + c} V{y + h} H{x + c} L{x},{y + h - c} Z"


def frame(w, h, code, kind="INTEL"):
    """Background, grid, corner brackets and the header strip."""
    return [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">',
        "<defs>"
        f'<pattern id="g" width="24" height="24" patternUnits="userSpaceOnUse"><path d="M24,0 H0 V24" fill="none" stroke="{GRID}" stroke-width="1"/></pattern>'
        f'<linearGradient id="fade" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="{BG}" stop-opacity="0.95"/><stop offset="0.55" stop-color="{BG}" stop-opacity="0.2"/><stop offset="1" stop-color="{BG}" stop-opacity="0"/></linearGradient>'
        "</defs>",
        f'<rect width="{w}" height="{h}" fill="{BG}"/>',
        f'<rect width="{w}" height="{h}" fill="url(#g)"/>',
        # header strip
        f'<rect x="0" y="0" width="{w}" height="26" fill="{PANEL}"/>',
        f'<rect x="0" y="25" width="{w}" height="1" fill="{LINE}"/>',
        f'<rect x="0" y="0" width="6" height="26" fill="{AMBER}"/>',
        t(16, 18, f"NIGHTGRID // {kind}", 12, AMBER, spacing=2, weight=700),
        t(w - 14, 18, code, 12, DIM, anchor="end", spacing=2),
        # corner ticks
        f'<path d="M{w - 30},{h - 8} H{w - 8} V{h - 30}" fill="none" stroke="{CYAN}" stroke-width="2"/>',
        f'<path d="M8,{h - 30} V{h - 8} H30" fill="none" stroke="{LINE}" stroke-width="2"/>',
    ]


def glitch(x, y, widths, color=MAGENTA):
    out, cx = [], x
    for i, wd in enumerate(widths):
        out.append(f'<rect x="{cx}" y="{y + (i % 3) * 5}" width="{wd}" height="2" fill="{color}" opacity="{0.35 + 0.2 * (i % 3)}"/>')
        cx += wd + 6
    return "".join(out)


# --- icons: line art centred on (cx, cy), about 2*s wide -------------------


def g(cx, cy, s, body, color=CYAN, sw=2):
    return f'<g transform="translate({cx},{cy}) scale({s / 40:.3f})" fill="none" stroke="{color}" stroke-width="{sw * 40 / s:.2f}" stroke-linejoin="round" stroke-linecap="round">{body}</g>'


def server(cx, cy, s=40, color=CYAN):
    body = "".join(f'<rect x="-34" y="{-36 + i * 25}" width="68" height="20" rx="2"/><circle cx="-22" cy="{-26 + i * 25}" r="2.5"/><path d="M-6,{-26 + i * 25} H24"/>' for i in range(3))
    return g(cx, cy, s, body, color)


def minipc(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-36" y="-14" width="72" height="28" rx="3"/><circle cx="24" cy="0" r="3"/><path d="M-26,0 H6"/><path d="M-30,14 v4 M30,14 v4"/>', color)


def nas(cx, cy, s=40, color=CYAN):
    bays = "".join(f'<rect x="{-28 + i * 15}" y="-24" width="11" height="40" rx="1"/><circle cx="{-22.5 + i * 15}" cy="10" r="1.5"/>' for i in range(4))
    return g(cx, cy, s, f'<rect x="-36" y="-34" width="72" height="62" rx="3"/>{bays}<path d="M-36,22 H36"/>', color)


def laptop(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-28" y="-26" width="56" height="36" rx="2"/><path d="M-38,16 H38 L32,24 H-32 Z"/><path d="M-18,-12 l6,5 -6,5 M-4,-2 H8"/>', color)


def phone(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-16" y="-32" width="32" height="64" rx="5"/><path d="M-5,24 H5"/>', color)


def router(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-36" y="0" width="72" height="22" rx="3"/><path d="M-24,0 L-30,-30 M24,0 L30,-30"/><circle cx="-20" cy="11" r="2"/><circle cx="-10" cy="11" r="2"/><circle cx="0" cy="11" r="2"/><path d="M12,11 H26"/>', color)


def cloud(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-26,18 H28 a14,14 0 0 0 0,-28 a20,20 0 0 0 -38,-8 a16,16 0 0 0 -16,36 Z"/>', color)


def shield(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M0,-34 L28,-24 V2 C28,20 14,30 0,36 C-14,30 -28,20 -28,2 V-24 Z"/><path d="M-10,0 l7,8 14,-16"/>', color)


def lock(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-22" y="-6" width="44" height="36" rx="3"/><path d="M-14,-6 V-16 a14,14 0 0 1 28,0 V-6"/><circle cx="0" cy="10" r="4"/><path d="M0,14 V20"/>', color)


def key(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<circle cx="-18" cy="0" r="12"/><circle cx="-18" cy="0" r="4"/><path d="M-6,0 H34 M22,0 V10 M30,0 V8"/>', color)


def disk(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<ellipse cx="0" cy="-20" rx="28" ry="9"/><path d="M-28,-20 V20 a28,9 0 0 0 56,0 V-20"/><path d="M-28,0 a28,9 0 0 0 56,0"/>', color)


def cube(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M0,-30 L28,-15 V15 L0,30 L-28,15 V-15 Z"/><path d="M-28,-15 L0,0 L28,-15 M0,0 V30"/>', color)


def gpu(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-38" y="-18" width="76" height="34" rx="3"/><circle cx="-14" cy="-1" r="10"/><circle cx="14" cy="-1" r="10"/><path d="M-30,16 V24 H6 V16"/>', color)


def gamepad(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-22,-14 H22 C34,-14 40,8 36,18 C33,25 25,24 20,16 L14,8 H-14 L-20,16 C-25,24 -33,25 -36,18 C-40,8 -34,-14 -22,-14 Z"/><path d="M-20,-4 V6 M-25,1 H-15"/><circle cx="18" cy="-2" r="2"/><circle cx="24" cy="4" r="2"/>', color)


def house(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-32,-2 L0,-30 L32,-2"/><path d="M-24,-8 V30 H24 V-8"/><rect x="-7" y="10" width="14" height="20"/>', color)


def bulb(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-10,18 C-10,8 -22,2 -22,-12 a22,22 0 0 1 44,0 C22,2 10,8 10,18 Z"/><path d="M-8,26 H8 M-5,32 H5"/>', color)


def wrench(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-30,30 L4,-4 a14,14 0 0 0 18,-18 l-8,8 -8,-2 -2,-8 8,-8 a14,14 0 0 0 -18,18 L-36,24 Z"/>', color)


def chip(cx, cy, s=40, color=CYAN):
    pins = "".join(f'<path d="M{p},-30 V-22 M{p},22 V30 M-30,{p} H-22 M22,{p} H30"/>' for p in (-12, 0, 12))
    return g(cx, cy, s, f'<rect x="-22" y="-22" width="44" height="44" rx="3"/><rect x="-10" y="-10" width="20" height="20"/>{pins}', color)


def doc(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<path d="M-22,-32 H10 L24,-18 V32 H-22 Z"/><path d="M10,-32 V-18 H24"/><path d="M-12,-6 H14 M-12,6 H14 M-12,18 H4"/>', color)


def photo(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-34" y="-26" width="68" height="52" rx="3"/><circle cx="-14" cy="-8" r="6"/><path d="M-34,20 L-8,0 L8,12 L18,4 L34,18"/>', color)


def film(cx, cy, s=40, color=CYAN):
    holes = "".join(f'<rect x="{-30 + i * 14}" y="-24" width="6" height="5"/><rect x="{-30 + i * 14}" y="19" width="6" height="5"/>' for i in range(5))
    return g(cx, cy, s, f'<rect x="-36" y="-30" width="72" height="60" rx="2"/>{holes}<path d="M-6,-10 L12,0 L-6,10 Z"/>', color)


def calendar(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-28" y="-24" width="56" height="54" rx="3"/><path d="M-28,-8 H28 M-14,-32 V-18 M14,-32 V-18"/><path d="M-16,6 H-8 M-2,6 H6 M12,6 H20 M-16,18 H-8 M-2,18 H6"/>', color)


def switch(cx, cy, s=40, color=CYAN):
    ports = "".join(f'<rect x="{-32 + i * 11}" y="-4" width="7" height="7"/>' for i in range(6))
    return g(cx, cy, s, f'<rect x="-38" y="-14" width="76" height="28" rx="3"/>{ports}', color)


def terminal(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-36" y="-26" width="72" height="52" rx="3"/><path d="M-36,-14 H36"/><path d="M-24,0 l8,6 -8,6 M-6,14 H12"/>', color)


def person(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<circle cx="0" cy="-16" r="12"/><path d="M-24,30 C-24,8 24,8 24,30"/>', color)


def tape(cx, cy, s=40, color=CYAN):
    return g(cx, cy, s, '<rect x="-36" y="-22" width="72" height="44" rx="3"/><circle cx="-14" cy="-2" r="8"/><circle cx="14" cy="-2" r="8"/><path d="M-14,-10 H14 M-22,22 L-16,12 H16 L22,22"/>', color)


ICONS = {
    "server": server, "minipc": minipc, "nas": nas, "laptop": laptop, "phone": phone, "router": router,
    "cloud": cloud, "shield": shield, "lock": lock, "key": key, "disk": disk, "cube": cube, "gpu": gpu,
    "gamepad": gamepad, "house": house, "bulb": bulb, "wrench": wrench, "chip": chip, "doc": doc,
    "photo": photo, "film": film, "calendar": calendar, "switch": switch, "terminal": terminal,
    "person": person, "tape": tape,
}


# --- diagram pieces ------------------------------------------------------------


def node(x, y, w, h, title, sub="", color=CYAN, icon=None, fill=PANEL):
    out = [f'<path d="{chamfer(x, y, w, h)}" fill="{fill}" stroke="{color}" stroke-width="1.5"/>', f'<rect x="{x}" y="{y}" width="4" height="{min(h, 22)}" fill="{color}"/>']
    tx = x + 14
    if icon:
        out.append(ICONS[icon](x + 28, y + h / 2, 18, color))
        tx = x + 54
    out.append(t(tx, y + (h / 2 + 5 if not sub else h / 2 - 3), title, 17, TEXT, weight=700))
    if sub:
        out.append(t(tx, y + h / 2 + 16, sub, 13, DIM))
    return "".join(out)


def wire(points, color=DIM, dash=False, arrow=True, label=None, lpos=None, lcolor=None):
    d = "M" + " L".join(f"{x},{y}" for x, y in points)
    da = ' stroke-dasharray="6 5"' if dash else ""
    out = [f'<path d="{d}" fill="none" stroke="{color}" stroke-width="1.8"{da}/>']
    if arrow:
        (x1, y1), (x2, y2) = points[-2], points[-1]
        import math

        a = math.atan2(y2 - y1, x2 - x1)
        p1 = (x2 - 10 * math.cos(a - 0.45), y2 - 10 * math.sin(a - 0.45))
        p2 = (x2 - 10 * math.cos(a + 0.45), y2 - 10 * math.sin(a + 0.45))
        out.append(f'<path d="M{p1[0]:.1f},{p1[1]:.1f} L{x2},{y2} L{p2[0]:.1f},{p2[1]:.1f}" fill="none" stroke="{color}" stroke-width="1.8"/>')
    if label:
        lx, ly = lpos or ((points[0][0] + points[-1][0]) / 2, (points[0][1] + points[-1][1]) / 2 - 8)
        out.append(t(lx, ly, label, 13, lcolor or color, anchor="middle"))
    return "".join(out)


def zone(x, y, w, h, label, color=LINE, lcolor=DIM):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="none" stroke="{color}" stroke-width="1.2" stroke-dasharray="3 4"/>' + t(x + 10, y + 17, label, 12, lcolor, spacing=1.5, weight=700)


def caption_bar(w, h, text):
    return f'<rect x="0" y="{h - 34}" width="{w}" height="34" fill="{PANEL}" opacity="0.92"/>' + t(18, h - 12, text, 14, MINT)


def finish(parts):
    return "".join(parts) + "</svg>\n"


# --- guide banners -------------------------------------------------------------


def banner(code, title, tagline, scene, objectives=()):
    """A guide's briefing banner: scene on the right, title block on the left."""
    w, h = 960, 312
    p = frame(w, h, code, "QUEST BRIEFING")
    p.append(f'<g transform="translate(70,0)">{scene}</g>')
    p.append(f'<rect x="0" y="26" width="{w}" height="{h - 26}" fill="url(#fade)"/>')
    p.append(t(32, 92, "ACTIVE QUEST", 12, MAGENTA, spacing=3, weight=700))
    p.append(glitch(172, 84, [18, 6, 30, 4, 12]))
    size = min(34, int(560 / (0.62 * len(title))))
    p.append(t(32, 132, title.upper(), size, TEXT, weight=700, family=SANS, spacing=1.5))
    p.append(f'<rect x="32" y="146" width="64" height="3" fill="{AMBER}"/>')
    words, lines = tagline.split(), [""]
    for wd in words:
        if len(lines[-1]) + len(wd) + 1 > 46:
            lines.append("")
        lines[-1] = f"{lines[-1]} {wd}".strip()
    for i, ln in enumerate(lines[:2]):
        p.append(t(32, 174 + i * 18, ln, 14, DIM))
    top = 174 + 18 * min(len(lines), 2) + 18
    for i, o in enumerate(objectives[:3]):
        y = top + i * 22
        p.append(f'<path d="M36,{y - 9} l6,6 -6,6 -6,-6 Z" fill="none" stroke="{AMBER}" stroke-width="1.5"/>')
        p.append(t(52, y + 1, o, 13, MINT))
    return finish(p)


def links(pairs, color=LINE, dash=True):
    return "".join(wire([a, b], color, dash=dash, arrow=False) for a, b in pairs)


def scene(items, pairs=(), extra=""):
    out = [links(pairs)]
    for name, x, y, s, color in items:
        out.append(ICONS[name](x, y, s, color))
    out.append(extra)
    return "".join(out)


def pulse(x, y, color=MAGENTA):
    return f'<circle cx="{x}" cy="{y}" r="5" fill="{color}"/><circle cx="{x}" cy="{y}" r="11" fill="none" stroke="{color}" stroke-width="1.2" opacity="0.6"/>'


GUIDES = {
    "guide-starter-homelab": ("QL-01", "First homelab weekend", "One machine, a few services, and the habits that keep it alive.",
        scene([("minipc", 640, 160, 56, CYAN), ("router", 820, 120, 40, DIM), ("laptop", 820, 230, 40, MINT), ("cube", 560, 250, 26, MAGENTA), ("cube", 640, 250, 26, CYAN), ("cube", 720, 250, 26, CYAN)],
              [((640, 160), (820, 120)), ((640, 160), (820, 230)), ((640, 160), (560, 250)), ((640, 160), (720, 250))], pulse(690, 140)),
        ("Install a server OS", "Add it to Nightgrid", "Run your first stacks")),
    "guide-mini-pc-homelab": ("QL-02", "Mini PC homelab", "Small, quiet, frugal: dozens of containers at a few watts.",
        scene([("minipc", 600, 130, 46, CYAN), ("minipc", 600, 220, 46, CYAN), ("switch", 800, 175, 46, DIM), ("bulb", 470, 175, 24, AMBER)],
              [((600, 130), (800, 175)), ((600, 220), (800, 175))], pulse(640, 120) + t(860, 250, "6-15 W IDLE", 12, AMBER, anchor="middle", spacing=2)),
        ("Pick the hardware", "Tune BIOS and power", "Bring it into Nightgrid")),
    "guide-diy-nas": ("QL-03", "Build a DIY NAS", "Your own storage box: pools, shares, snapshots and real backups.",
        scene([("nas", 650, 160, 60, CYAN), ("disk", 820, 110, 26, MINT), ("disk", 820, 180, 26, MINT), ("disk", 820, 250, 26, MINT), ("laptop", 500, 240, 30, DIM)],
              [((650, 160), (820, 110)), ((650, 160), (820, 180)), ((650, 160), (820, 250)), ((650, 160), (500, 240))], pulse(690, 120)),
        ("Build the pool", "Share it on the LAN", "Snapshot and back up")),
    "guide-family-cloud-photos": ("QL-04", "Family cloud and photos", "Everyone's photos and files, on hardware you own.",
        scene([("cloud", 680, 120, 50, CYAN), ("photo", 560, 220, 34, MINT), ("phone", 700, 235, 26, MINT), ("phone", 790, 225, 26, MINT), ("house", 860, 110, 34, DIM)],
              [((680, 120), (560, 220)), ((680, 120), (700, 235)), ((680, 120), (790, 225)), ((680, 120), (860, 110))], pulse(720, 100)),
        ("Set up the photo server", "Connect every phone", "Back up the originals")),
    "guide-game-server-host": ("QL-05", "Game server host", "Worlds for your friends that stay up when you log off.",
        scene([("server", 640, 160, 56, CYAN), ("gamepad", 830, 110, 32, MAGENTA), ("gamepad", 830, 215, 32, MINT), ("shield", 500, 230, 26, AMBER)],
              [((640, 160), (830, 110)), ((640, 160), (830, 215))], pulse(680, 120)),
        ("Host the servers", "Open the right ports", "Back up the worlds")),
    "guide-local-ai-gpu-server": ("QL-06", "Local AI GPU server", "Models on your own GPU: private, offline, and yours.",
        scene([("gpu", 650, 150, 60, CYAN), ("chip", 830, 120, 30, MAGENTA), ("terminal", 830, 225, 32, MINT), ("server", 500, 230, 30, DIM)],
              [((650, 150), (830, 120)), ((650, 150), (830, 225)), ((650, 150), (500, 230))], pulse(700, 110)),
        ("Drivers and toolkit", "Run the model server", "Give it a web UI")),
    "guide-proxmox-media-server": ("QL-07", "Proxmox media server", "VMs, containers and a media stack, run from Nightgrid.",
        scene([("server", 620, 160, 56, CYAN), ("film", 800, 110, 32, MAGENTA), ("cube", 800, 210, 28, MINT), ("nas", 500, 240, 28, DIM)],
              [((620, 160), (800, 110)), ((620, 160), (800, 210)), ((620, 160), (500, 240))], pulse(660, 120)),
        ("Install Proxmox", "Build the media stack", "Pass the GPU through")),
    "guide-remote-family-it": ("QL-08", "Remote family IT", "Help the family's devices from wherever you are.",
        scene([("laptop", 560, 150, 40, CYAN), ("house", 760, 110, 38, MINT), ("house", 860, 200, 32, MINT), ("lock", 670, 240, 24, AMBER)],
              [((560, 150), (760, 110)), ((560, 150), (860, 200))], pulse(600, 130)),
        ("Join them to the tailnet", "Remote desktop on call", "Backups they never see")),
    "guide-repair-bench-server": ("QL-09", "Repair bench server", "Images, tools and logs for a workshop bench.",
        scene([("server", 620, 150, 50, CYAN), ("wrench", 800, 110, 30, AMBER), ("laptop", 820, 220, 32, MINT), ("disk", 500, 230, 24, DIM)],
              [((620, 150), (800, 110)), ((620, 150), (820, 220)), ((620, 150), (500, 230))], pulse(660, 110)),
        ("Network boot the bench", "Test and wipe drives", "Track every job")),
    "guide-smart-home-hub": ("QL-10", "Smart home hub", "Lights, sensors and automations that work without the cloud.",
        scene([("house", 650, 150, 56, CYAN), ("bulb", 830, 100, 26, AMBER), ("chip", 840, 200, 24, MINT), ("phone", 520, 230, 24, DIM)],
              [((650, 150), (830, 100)), ((650, 150), (840, 200)), ((650, 150), (520, 230))], pulse(690, 110)),
        ("Home Assistant up", "Pair Zigbee devices", "Automate, locally")),
    "guide-dev-git-deploy-lab": ("QL-11", "Dev and git deploy lab", "Push code, run CI, deploy stacks: your own pipeline.",
        scene([("terminal", 560, 150, 40, CYAN), ("cube", 700, 120, 30, MAGENTA), ("server", 840, 170, 40, MINT), ("doc", 700, 230, 24, DIM)],
              [((560, 150), (700, 120)), ((700, 120), (840, 170)), ((700, 230), (700, 120))], pulse(600, 130)),
        ("Host your git", "Add CI runners", "Deploy from git")),
    "guide-10gbe-upgrade": ("QL-12", "Upgrade the homelab to 10 GbE", "Fast links where they count, without melting the budget.",
        scene([("switch", 680, 130, 56, CYAN), ("server", 560, 230, 30, MINT), ("nas", 680, 240, 30, MINT), ("laptop", 810, 235, 30, DIM)],
              [((680, 130), (560, 230)), ((680, 130), (680, 240)), ((680, 130), (810, 235))], t(780, 105, "10G", 26, MAGENTA, anchor="middle", weight=700, family=SANS) + pulse(720, 110)),
        ("Choose NICs and cables", "Wire the core", "Prove the speed")),
    "guide-small-office-server": ("QL-13", "Small office server", "One box for a small shop: files, passwords, calendars, backups.",
        scene([("server", 620, 150, 50, CYAN), ("person", 790, 100, 22, MINT), ("person", 850, 140, 22, MINT), ("calendar", 820, 230, 26, AMBER), ("doc", 500, 230, 24, DIM)],
              [((620, 150), (790, 100)), ((620, 150), (850, 140)), ((620, 150), (820, 230)), ((620, 150), (500, 230))], pulse(660, 110)),
        ("Base system and proxy", "The daily tools", "Backups and alerts")),
}


# --- diagrams --------------------------------------------------------------------


def fig_reverse_proxy():
    w, h = 960, 380
    p = frame(w, h, "DX-01")
    p.append(zone(250, 50, 690, 290, "YOUR SERVER"))
    p.append(node(30, 160, 200, 60, "Browser", "app.example.com", MINT, "laptop"))
    p.append(node(290, 160, 200, 60, "Reverse proxy", ":443 · TLS certs", AMBER, "shield"))
    for i, (n, port) in enumerate([("Nextcloud", "127.0.0.1:8080"), ("Jellyfin", "127.0.0.1:8096"), ("Vaultwarden", "127.0.0.1:8081")]):
        y = 80 + i * 85
        p.append(node(640, y, 270, 58, n, port, CYAN, "cube"))
        p.append(wire([(490, 190), (565, 190), (565, y + 29), (640, y + 29)], CYAN))
    p.append(wire([(230, 190), (290, 190)], MAGENTA, label="HTTPS", lcolor=MAGENTA))
    p.append(t(565, 322, "picks the app by host name", 11, DIM, anchor="middle"))
    p.append(caption_bar(w, h, "Only 80/443 face the network; apps listen on 127.0.0.1 and the proxy adds TLS."))
    return finish(p)


def fig_backups_321():
    w, h = 960, 380
    p = frame(w, h, "DX-02")
    p.append(node(40, 150, 220, 70, "Live data", "server disks", CYAN, "server"))
    p.append(node(370, 70, 240, 66, "Copy 2", "other disk / NAS", MINT, "nas"))
    p.append(node(370, 230, 240, 66, "Copy 3", "off-site, encrypted", MAGENTA, "cloud"))
    p.append(wire([(260, 185), (315, 185), (315, 103), (370, 103)], MINT, label="nightly", lpos=(300, 95)))
    p.append(wire([(260, 185), (315, 185), (315, 263), (370, 263)], MAGENTA, label="nightly", lpos=(300, 285)))
    p.append(node(690, 150, 245, 70, "Test restore", "monthly, scheduled", AMBER, "wrench"))
    p.append(wire([(610, 263), (650, 263), (650, 185), (690, 185)], AMBER, dash=True))
    for i, (big, small) in enumerate([("3", "copies"), ("2", "media"), ("1", "off-site")]):
        x = 720 + i * 72
        p.append(t(x, 300, big, 30, AMBER, anchor="middle", weight=700, family=SANS))
        p.append(t(x, 318, small, 11, DIM, anchor="middle"))
    p.append(caption_bar(w, h, "3-2-1: three copies, two kinds of storage, one off-site. A backup counts once a restore works."))
    return finish(p)


def fig_tailnet():
    w, h = 960, 400
    p = frame(w, h, "DX-03")
    p.append(zone(30, 50, 300, 300, "ANYWHERE"))
    p.append(node(55, 90, 250, 58, "Laptop", "tailnet 100.x", MINT, "laptop"))
    p.append(node(55, 200, 250, 58, "Phone", "tailnet 100.x", MINT, "phone"))
    p.append(zone(560, 50, 370, 300, "HOME LAN 192.168.1.0/24"))
    p.append(node(590, 80, 310, 60, "Subnet router", "advertises the LAN route", AMBER, "router"))
    p.append(node(590, 180, 145, 56, "NAS", ".20", CYAN, "nas"))
    p.append(node(755, 180, 145, 56, "Printer", ".30", CYAN, "doc"))
    p.append(node(590, 270, 310, 56, "Server with Tailscale", "reached directly", CYAN, "server"))
    p.append(wire([(305, 119), (590, 110)], MAGENTA, label="encrypted (WireGuard)", lcolor=MAGENTA))
    p.append(wire([(305, 229), (430, 229), (430, 298), (590, 298)], MAGENTA, label="direct", lpos=(500, 290), lcolor=MAGENTA))
    p.append(wire([(745, 140), (745, 160), (662, 160), (662, 180)], CYAN, arrow=True))
    p.append(wire([(745, 160), (827, 160), (827, 180)], CYAN))
    p.append(caption_bar(w, h, "No open ports: devices join the tailnet; a subnet router reaches LAN gear that can't run Tailscale."))
    return finish(p)


def fig_ssh_keys():
    w, h = 960, 360
    p = frame(w, h, "DX-04")
    p.append(zone(30, 50, 380, 260, "YOUR COMPUTER"))
    p.append(node(55, 90, 330, 64, "Private key", "~/.ssh/id_ed25519 · never leaves", MAGENTA, "key"))
    p.append(node(55, 200, 330, 64, "Public key", "~/.ssh/id_ed25519.pub · share it", MINT, "key"))
    p.append(zone(550, 50, 380, 260, "EACH SERVER"))
    p.append(node(575, 90, 330, 64, "sshd", "PasswordAuthentication no", AMBER, "lock"))
    p.append(node(575, 200, 330, 64, "authorized_keys", "~/.ssh/authorized_keys", CYAN, "doc"))
    p.append(wire([(385, 122), (575, 122)], MAGENTA, label="signs in", lcolor=MAGENTA))
    p.append(wire([(385, 232), (575, 232)], MINT, label="copy once"))
    p.append(wire([(740, 154), (740, 200)], AMBER, arrow=True))
    p.append(t(752, 182, "checks against", 11, AMBER))
    p.append(caption_bar(w, h, "The server checks a signature made by your private key against the public key it holds."))
    return finish(p)


def fig_compose():
    w, h = 960, 380
    p = frame(w, h, "DX-05")
    p.append(node(30, 90, 230, 60, "compose.yaml", "services and ports", AMBER, "doc"))
    p.append(node(30, 190, 230, 60, ".env", "secrets (mode 600)", MAGENTA, "lock"))
    p.append(zone(320, 50, 610, 290, "DOCKER HOST · project network"))
    p.append(node(350, 90, 240, 60, "app", "image: vendor/app:1.4", CYAN, "cube"))
    p.append(node(350, 220, 240, 60, "db", "image: postgres:16", CYAN, "cube"))
    p.append(node(670, 90, 230, 60, "Port", "127.0.0.1:8080 → 80", MINT, "router"))
    p.append(node(670, 220, 230, 60, "Volumes", "./data  ./db", MINT, "disk"))
    p.append(wire([(260, 120), (350, 120)], AMBER, label="up", lcolor=AMBER))
    p.append(wire([(260, 220), (300, 220), (300, 250), (350, 250)], MAGENTA))
    p.append(wire([(470, 150), (470, 220)], CYAN, label="db:5432", lpos=(510, 190)))
    p.append(wire([(590, 120), (670, 120)], MINT))
    p.append(wire([(590, 250), (670, 250)], MINT))
    p.append(caption_bar(w, h, "Containers are disposable; everything worth keeping lives in volumes and .env."))
    return finish(p)


def fig_docker_networks():
    w, h = 960, 400
    p = frame(w, h, "DX-06")
    p.append(zone(30, 50, 280, 300, "DEFAULT BRIDGE (per stack)"))
    p.append(node(55, 100, 230, 54, "app", "172.18.0.2", CYAN, "cube"))
    p.append(node(55, 220, 230, 54, "db", "172.18.0.3", CYAN, "cube"))
    p.append(wire([(170, 154), (170, 220)], CYAN, label="by name: db", lpos=(220, 192)))
    p.append(zone(340, 50, 280, 300, "SHARED 'proxy' NETWORK"))
    p.append(node(365, 90, 230, 54, "Caddy / Traefik", ":80 :443", AMBER, "shield"))
    p.append(node(365, 190, 230, 54, "app A", "joins proxy", CYAN, "cube"))
    p.append(node(365, 270, 230, 54, "app B", "joins proxy", CYAN, "cube"))
    p.append(wire([(480, 144), (480, 190)], AMBER))
    p.append(wire([(595, 117), (608, 117), (608, 297), (595, 297)], AMBER, dash=True))
    p.append(zone(650, 50, 280, 300, "MACVLAN"))
    p.append(node(675, 100, 230, 54, "Pi-hole", "192.168.1.53 (LAN)", MAGENTA, "cube"))
    p.append(node(675, 220, 230, 54, "LAN devices", "see it as a machine", MINT, "router"))
    p.append(wire([(790, 220), (790, 154)], MAGENTA))
    p.append(caption_bar(w, h, "Bridge for a stack's own services, a shared network for the proxy, macvlan for a LAN address."))
    return finish(p)


def fig_arr_hardlinks():
    w, h = 960, 380
    p = frame(w, h, "DX-07")
    p.append(zone(30, 50, 900, 280, "ONE FILESYSTEM: /data"))
    p.append(node(60, 90, 250, 60, "Download client", "/data/torrents/tv", AMBER, "cloud"))
    p.append(node(60, 230, 250, 60, "Sonarr / Radarr", "watch and import", CYAN, "cube"))
    p.append(node(400, 160, 230, 60, "Hardlink", "same blocks, 2 names", MAGENTA, "disk"))
    p.append(node(690, 90, 235, 60, "Library", "/data/media/tv", MINT, "film"))
    p.append(node(690, 230, 235, 60, "Jellyfin / Plex", "reads /data/media", MINT, "film"))
    p.append(wire([(185, 150), (185, 230)], AMBER, label="done", lpos=(215, 195)))
    p.append(wire([(310, 260), (360, 260), (360, 190), (400, 190)], CYAN))
    p.append(wire([(630, 190), (665, 190), (665, 120), (690, 120)], MAGENTA, label="instant", lpos=(640, 110), lcolor=MAGENTA))
    p.append(wire([(807, 150), (807, 230)], MINT))
    p.append(caption_bar(w, h, "Mount one /data into every container so imports are hardlinks: no copy, no double space."))
    return finish(p)


def fig_jump_host():
    w, h = 960, 360
    p = frame(w, h, "DX-08")
    p.append(node(40, 150, 210, 64, "Nightgrid", "your computer", MINT, "laptop"))
    p.append(node(360, 150, 220, 64, "Jump host", "reachable from you", AMBER, "server"))
    p.append(zone(660, 50, 270, 270, "PRIVATE LAN"))
    for i, n in enumerate(["NAS", "VM host", "Pi"]):
        y = 80 + i * 78
        p.append(node(690, y, 210, 56, n, f"192.168.1.{20 + i * 10}", CYAN, ["nas", "server", "chip"][i]))
        p.append(wire([(580, 182), (630, 182), (630, y + 28), (690, y + 28)], CYAN))
    p.append(wire([(250, 182), (360, 182)], MAGENTA, label="SSH", lcolor=MAGENTA))
    p.append(caption_bar(w, h, "One SSH hop: Nightgrid tunnels through the jump host; your key is checked at every hop."))
    return finish(p)


def fig_zfs_layout():
    w, h = 960, 400
    p = frame(w, h, "DX-09")
    p.append(node(30, 160, 200, 64, "Pool: tank", "zpool create", AMBER, "nas"))
    p.append(zone(270, 50, 250, 300, "VDEV: mirror-0"))
    p.append(node(295, 100, 200, 56, "Disk A", "by-id path", CYAN, "disk"))
    p.append(node(295, 240, 200, 56, "Disk B", "same data", CYAN, "disk"))
    p.append(wire([(230, 192), (262, 192), (262, 128), (295, 128)], AMBER))
    p.append(wire([(262, 192), (262, 268), (295, 268)], AMBER))
    p.append(zone(560, 50, 370, 300, "DATASETS"))
    for i, (n, sub) in enumerate([("tank/share", "SMB share"), ("tank/media", "recordsize=1M"), ("tank/backups", "replicated")]):
        y = 80 + i * 86
        p.append(node(585, y, 210, 58, n, sub, MINT, "doc"))
        p.append(wire([(520, 200), (545, 200), (545, y + 29), (585, y + 29)], MINT))
        p.append(t(830, y + 24, "@hourly", 13, MAGENTA))
        p.append(t(830, y + 42, "@daily", 13, MAGENTA))
    p.append(caption_bar(w, h, "A pool is built from vdevs; datasets share its space and each gets its own snapshots."))
    return finish(p)


def fig_wireguard_hub():
    w, h = 960, 380
    p = frame(w, h, "DX-10")
    p.append(node(360, 150, 250, 70, "WireGuard server", "10.8.0.1 · UDP 51820", AMBER, "server"))
    p.append(node(30, 70, 220, 60, "Router", "forwards 51820", DIM, "router"))
    p.append(wire([(250, 100), (300, 100), (300, 172), (360, 172)], DIM))
    for i, (n, ip, ic) in enumerate([("Phone", "10.8.0.2", "phone"), ("Laptop", "10.8.0.3", "laptop"), ("Friend's PC", "10.8.0.4", "laptop")]):
        y = 60 + i * 95
        p.append(node(700, y, 230, 60, n, ip, MINT, ic))
        p.append(wire([(700, y + 30), (650, y + 30), (650, 185), (610, 185)], MAGENTA))
    p.append(node(30, 230, 220, 60, "Home LAN", "192.168.1.0/24", CYAN, "house"))
    p.append(wire([(360, 200), (300, 200), (300, 260), (250, 260)], CYAN, label="NAT", lpos=(320, 245)))
    p.append(caption_bar(w, h, "Peers dial the server; it routes them to each other and, with NAT, to the home LAN."))
    return finish(p)


def fig_dns_chain():
    w, h = 960, 360
    p = frame(w, h, "DX-11")
    p.append(node(30, 140, 190, 64, "Devices", "DNS from DHCP", MINT, "phone"))
    p.append(node(280, 140, 200, 64, "Pi-hole", "blocks ads :53", MAGENTA, "shield"))
    p.append(node(540, 140, 190, 64, "Unbound", "127.0.0.1:5335", AMBER, "server"))
    p.append(node(780, 60, 150, 56, "Root", "the . zone", CYAN, "cloud"))
    p.append(node(780, 145, 150, 56, "TLD", ".com .org", CYAN, "cloud"))
    p.append(node(780, 230, 150, 56, "Domain", "example.com", CYAN, "cloud"))
    p.append(wire([(220, 172), (280, 172)], MINT))
    p.append(wire([(480, 172), (540, 172)], MAGENTA))
    for y in (88, 173, 258):
        p.append(wire([(730, 172), (755, 172), (755, y), (780, y)], AMBER))
    p.append(t(380, 240, "blocked → 0.0.0.0", 13, MAGENTA, anchor="middle"))
    p.append(caption_bar(w, h, "Pi-hole filters, Unbound asks the authoritative servers itself: no upstream provider sees it all."))
    return finish(p)


def fig_vlans():
    w, h = 960, 380
    p = frame(w, h, "DX-12")
    p.append(node(30, 150, 170, 66, "Switch", "trunk port", DIM, "switch"))
    p.append(node(330, 150, 220, 66, "Server NIC", "eth0 (untagged: LAN)", CYAN, "server"))
    p.append(wire([(200, 183), (330, 183)], AMBER, label="tags 20, 30", lcolor=AMBER))
    for i, (n, sub, c) in enumerate([("eth0", "LAN 192.168.1.10", CYAN), ("vlan20", "IoT 192.168.20.10", MINT), ("vlan30", "Guests 192.168.30.10", MAGENTA)]):
        y = 60 + i * 95
        p.append(node(660, y, 270, 60, n, sub, c, "router"))
        p.append(wire([(550, 183), (605, 183), (605, y + 30), (660, y + 30)], c))
    p.append(caption_bar(w, h, "One cable carries several networks; the server gets one interface per VLAN tag."))
    return finish(p)


def fig_cloudflared():
    w, h = 960, 360
    p = frame(w, h, "DX-13")
    p.append(node(30, 140, 190, 64, "Visitor", "app.example.com", MINT, "laptop"))
    p.append(node(270, 140, 235, 64, "Cloudflare edge", "TLS, WAF, Access", AMBER, "cloud"))
    p.append(zone(560, 50, 370, 260, "HOME (no open ports)"))
    p.append(node(585, 90, 320, 60, "cloudflared", "outbound connection", MAGENTA, "cube"))
    p.append(node(585, 210, 320, 60, "App", "http://app:8080", CYAN, "cube"))
    p.append(wire([(220, 172), (270, 172)], MINT))
    p.append(wire([(585, 120), (530, 120), (530, 160), (505, 160)], MAGENTA, label="dials out", lpos=(530, 112), lcolor=MAGENTA))
    p.append(wire([(745, 150), (745, 210)], CYAN))
    p.append(caption_bar(w, h, "The tunnel is opened from inside, so the router forwards nothing; Cloudflare sees the traffic."))
    return finish(p)


def fig_gpu_passthrough():
    w, h = 960, 380
    p = frame(w, h, "DX-14")
    p.append(zone(30, 50, 420, 290, "HOST (Proxmox / libvirt)"))
    p.append(node(55, 90, 370, 60, "IOMMU on", "intel_iommu=on / AMD-Vi", AMBER, "chip"))
    p.append(node(55, 175, 370, 60, "vfio-pci claims the GPU", "ids=10de:xxxx,10de:yyyy", MAGENTA, "gpu"))
    p.append(node(55, 260, 370, 56, "Host driver blacklisted", "nouveau / nvidia / amdgpu", DIM, "lock"))
    p.append(wire([(240, 150), (240, 175)], AMBER))
    p.append(zone(540, 50, 390, 290, "VM"))
    p.append(node(565, 120, 340, 60, "GPU as a PCI device", "hostpci0 / <hostdev>", CYAN, "gpu"))
    p.append(node(565, 220, 340, 60, "Guest driver", "games, AI, transcoding", MINT, "terminal"))
    p.append(wire([(425, 205), (490, 205), (490, 150), (565, 150)], MAGENTA, label="whole group", lpos=(490, 140), lcolor=MAGENTA))
    p.append(wire([(735, 180), (735, 220)], CYAN))
    p.append(caption_bar(w, h, "The host gives the GPU (and everything in its IOMMU group) to one VM, exclusively."))
    return finish(p)


def fig_lvm():
    w, h = 960, 360
    p = frame(w, h, "DX-15")
    for i, n in enumerate(["/dev/sda3", "/dev/sdb"]):
        p.append(node(30, 80 + i * 120, 200, 60, "PV", n, CYAN, "disk"))
        p.append(wire([(230, 110 + i * 120), (300, 110 + i * 120), (300, 170), (360, 170)], CYAN))
    p.append(node(360, 138, 220, 64, "VG: vg0", "pooled space", AMBER, "nas"))
    for i, (n, sub) in enumerate([("LV: root", "/  ext4"), ("LV: data", "/srv  xfs"), ("snapshot", "before upgrades")]):
        y = 60 + i * 90
        c = MAGENTA if i == 2 else MINT
        p.append(node(680, y, 250, 58, n, sub, c, "doc"))
        p.append(wire([(580, 170), (630, 170), (630, y + 29), (680, y + 29)], c))
    p.append(caption_bar(w, h, "Disks become PVs, PVs pool into a VG, and LVs are carved out, grown or snapshotted."))
    return finish(p)


def fig_nut():
    w, h = 960, 380
    p = frame(w, h, "DX-16")
    p.append(node(30, 150, 190, 66, "UPS", "USB to one server", AMBER, "tape"))
    p.append(node(300, 150, 260, 66, "NUT server", "upsd :3493 · primary", MAGENTA, "server"))
    p.append(wire([(220, 183), (300, 183)], AMBER, label="USB", lcolor=AMBER))
    for i, (n, ic) in enumerate([("NAS", "nas"), ("VM host", "server"), ("Nightgrid", "laptop")]):
        y = 60 + i * 95
        sub = "reads status, alerts" if n == "Nightgrid" else "upsmon secondary"
        p.append(node(680, y, 250, 60, n, sub, MINT if n == "Nightgrid" else CYAN, ic))
        p.append(wire([(560, 183), (620, 183), (620, y + 30), (680, y + 30)], CYAN))
    p.append(t(300, 260, "low battery: secondaries shut down,", 13, DIM))
    p.append(t(300, 278, "then the primary, then the UPS", 13, DIM))
    p.append(caption_bar(w, h, "One machine talks to the UPS; every other machine listens over the network."))
    return finish(p)


def fig_prometheus():
    w, h = 960, 380
    p = frame(w, h, "DX-17")
    for i, (n, sub) in enumerate([("node_exporter", "each host :9100"), ("cAdvisor", "containers :8080"), ("app /metrics", "any exporter")]):
        y = 60 + i * 95
        p.append(node(30, y, 250, 60, n, sub, CYAN, "chip"))
        p.append(wire([(380, 183), (330, 183), (330, y + 30), (280, y + 30)], AMBER))
    p.append(node(380, 150, 220, 66, "Prometheus", "scrapes every 15 s", AMBER, "server"))
    p.append(node(690, 90, 240, 60, "Grafana", "dashboards", MINT, "photo"))
    p.append(node(690, 220, 240, 60, "Alertmanager", "rules → notify", MAGENTA, "bulb"))
    p.append(wire([(690, 120), (640, 120), (640, 183), (600, 183)], MINT, label="PromQL", lpos=(640, 110)))
    p.append(wire([(600, 190), (640, 190), (640, 250), (690, 250)], MAGENTA))
    p.append(caption_bar(w, h, "Prometheus pulls metrics from exporters; Grafana queries it, Alertmanager sends its alerts."))
    return finish(p)


# --- layouts for many small diagrams ---------------------------------------------
# Most articles need one of three shapes, so they are described as data:
#   flow(...)      boxes in columns with arrows between them
#   stack(...)     layers from top to bottom (boot chains, storage stacks)
#   steps(...)     a numbered procedure from left to right
# Each layout warns on stderr when a label would spill out of its box.

COLORS = {"c": CYAN, "m": MAGENTA, "a": AMBER, "t": MINT, "d": DIM}
W = 960


def _warn(name, msg):
    print(f"  ! {name}: {msg}", file=sys.stderr)


def _fits(name, s, size, room):
    width = len(s) * (0.62 * size if size >= 15 else 0.61 * size)
    if width > room:
        _warn(name, f"{s!r} needs {width:.0f}px, has {room:.0f}px")


def flow(code, caption, cols, edges=(), zones=(), gap=80, nh=60, row=88, name=""):
    """cols: [[(id, title, sub, color, icon), ...], ...]; edges: (a, b[, label[, color[, dash[, arrow]]]]);
    zones: (label, [ids]) draws a dashed box around those nodes."""
    n = len(cols)
    nw = min(270, (W - 60 - (n - 1) * gap) / n)
    total = n * nw + (n - 1) * gap
    x0 = (W - total) / 2
    rows = max(len(c) for c in cols)
    top = 62 + (24 if zones else 0)
    h = max(250, top + rows * row + 58)
    mid = (top + h - 50) / 2
    pos = {}
    for ci, col in enumerate(cols):
        y0 = mid - (len(col) * row - (row - nh)) / 2
        for ri, nd in enumerate(col):
            pos[nd[0]] = (x0 + ci * (nw + gap), y0 + ri * row, ci)
    p = frame(W, h, code)
    for label, ids in zones:
        xs = [pos[i][0] for i in ids]
        ys = [pos[i][1] for i in ids]
        p.append(zone(min(xs) - 14, min(ys) - 30, max(xs) - min(xs) + nw + 28, max(ys) - min(ys) + nh + 44, label))
    for e in edges:
        a, b = e[0], e[1]
        label = e[2] if len(e) > 2 else None
        color = COLORS.get(e[3], e[3]) if len(e) > 3 else DIM
        dash = e[4] if len(e) > 4 else False
        arrow = e[5] if len(e) > 5 else True
        xa, ya, ca = pos[a]
        xb, yb, cb = pos[b]
        if cb > ca:
            sx, sy, tx, ty = xa + nw, ya + nh / 2, xb, yb + nh / 2
            mx = tx - gap / 2
            pts = [(sx, sy), (tx, ty)] if abs(sy - ty) < 1 else [(sx, sy), (mx, sy), (mx, ty), (tx, ty)]
            lpos = (mx, min(sy, ty) - 8) if len(pts) == 4 else ((sx + tx) / 2, ty - 8)
            room = gap
        elif cb < ca:
            sx, sy, tx, ty = xa, ya + nh / 2, xb + nw, yb + nh / 2
            mx = tx + gap / 2
            pts = [(sx, sy), (tx, ty)] if abs(sy - ty) < 1 else [(sx, sy), (mx, sy), (mx, ty), (tx, ty)]
            lpos = (mx, min(sy, ty) - 8) if len(pts) == 4 else ((sx + tx) / 2, ty - 8)
            room = gap
        else:
            cx = xa + nw / 2
            pts = [(cx, ya + nh), (cx, yb)] if yb > ya else [(cx, ya), (cx, yb + nh)]
            lpos = (cx + 10, (pts[0][1] + pts[1][1]) / 2 + 4)
            room = 400
        if label and cb == ca:
            p.append(wire(pts, color, dash, arrow))
            p.append(t(lpos[0], lpos[1], label, 12, color))
        else:
            if label:
                _fits(name, label, 13, room * 2 - 6)
            p.append(wire(pts, color, dash, arrow, label=label, lpos=lpos))
    for col in cols:
        for nd in col:
            i, title, sub, color, icon = (list(nd) + [None] * 5)[:5]
            x, y, _ = pos[i]
            def need(size, txt):
                return len(txt or "") * 0.62 * size
            if icon and max(need(17, title), need(13, sub)) > nw - 62 and max(need(17, title), need(13, sub)) <= nw - 22:
                icon = None  # narrow box: the words matter more than the icon
            room = nw - (54 if icon else 14) - 8
            _fits(name, title, 17, room)
            _fits(name, sub or "", 13, room)
            p.append(node(x, y, nw, nh, title, sub or "", COLORS.get(color, color or CYAN), icon))
    _fits(name, caption, 14, W - 40)
    p.append(caption_bar(W, h, caption))
    return finish(p)


def stack(code, caption, layers, note=None, name=""):
    """layers: [(title, sub, color, icon), ...] from top to bottom; note: text beside the arrow."""
    lh, gapy = 52, 16
    h = 60 + len(layers) * (lh + gapy) + 50
    p = frame(W, h, code)
    x, w = 150, 660
    for i, (title, sub, color, icon) in enumerate(layers):
        y = 50 + i * (lh + gapy)
        col = COLORS.get(color, color)
        p.append(f'<path d="{chamfer(x, y, w, lh)}" fill="{PANEL}" stroke="{col}" stroke-width="1.5"/>')
        p.append(f'<rect x="{x}" y="{y}" width="4" height="{lh}" fill="{col}"/>')
        if icon:
            p.append(ICONS[icon](x + 30, y + lh / 2, 18, col))
        p.append(t(x + 58, y + lh / 2 + 6, title, 17, TEXT, weight=700))
        p.append(t(x + w - 16, y + lh / 2 + 5, sub, 13, DIM, anchor="end"))
        _fits(name, title + "  " + sub, 15, w - 80)
        p.append(t(x - 18, y + lh / 2 + 5, f"{i + 1:02d}", 13, col, anchor="end", weight=700))
        if i:
            p.append(f'<path d="M{x + w / 2},{y - gapy + 2} V{y - 2}" stroke="{LINE}" stroke-width="2"/>')
    y1, y2 = 50 + lh / 2, 50 + (len(layers) - 1) * (lh + gapy) + lh / 2
    p.append(wire([(x + w + 40, y1), (x + w + 40, y2)], AMBER))
    if note:
        p.append(f'<text x="{x + w + 64}" y="{(y1 + y2) / 2}" font-family="{MONO}" font-size="12" fill="{AMBER}" text-anchor="middle" transform="rotate(90 {x + w + 64} {(y1 + y2) / 2})">{escape(note)}</text>')
    _fits(name, caption, 14, W - 40)
    p.append(caption_bar(W, h, caption))
    return finish(p)


def _wrap(s, width):
    words, lines, cur = s.split(), [], ""
    for wd in words:
        if cur and len(cur) + 1 + len(wd) > width:
            lines.append(cur)
            cur = wd
        else:
            cur = f"{cur} {wd}".strip()
    if cur:
        lines.append(cur)
    return lines


def steps(code, caption, items, warn_at=None, name=""):
    """items: [(title, detail), ...] left to right; warn_at: index drawn in magenta (the risky step)."""
    n = len(items)
    sw = (W - 60) / n
    chars = int((sw - 20) / 7.9)
    details = [_wrap(d, chars) for _, d in items]
    h = 150 + max(len(d) for d in details) * 17 + 50
    p = frame(W, h, code)
    p.append(f'<rect x="{30 + sw / 2}" y="69" width="{sw * (n - 1)}" height="2" fill="{LINE}"/>')
    for i, (title, _) in enumerate(items):
        cx = 30 + sw * i + sw / 2
        col = MAGENTA if warn_at == i else (AMBER if i == n - 1 else CYAN)
        p.append(f'<path d="M{cx},{52} L{cx + 18},{70} L{cx},{88} L{cx - 18},{70} Z" fill="{BG}" stroke="{col}" stroke-width="2"/>')
        p.append(t(cx, 75, f"{i + 1:02d}", 13, col, anchor="middle", weight=700))
        _fits(name, title, 16, sw - 10)
        p.append(t(cx, 116, title, 16, TEXT, anchor="middle", weight=700))
        for li, line in enumerate(details[i]):
            p.append(t(cx, 140 + li * 17, line, 13, DIM, anchor="middle"))
    _fits(name, caption, 14, W - 40)
    p.append(caption_bar(W, h, caption))
    return finish(p)



FIGURES = {
    **{name: (lambda v=v: banner(*v)) for name, v in GUIDES.items()},
    "reverse-proxy": fig_reverse_proxy,
    "backups-321": fig_backups_321,
    "tailnet-subnet": fig_tailnet,
    "ssh-keys": fig_ssh_keys,
    "compose-anatomy": fig_compose,
    "docker-networks": fig_docker_networks,
    "arr-hardlinks": fig_arr_hardlinks,
    "jump-host": fig_jump_host,
    "zfs-layout": fig_zfs_layout,
    "wireguard-hub": fig_wireguard_hub,
    "dns-chain": fig_dns_chain,
    "vlans-trunk": fig_vlans,
    "cloudflared-tunnel": fig_cloudflared,
    "gpu-passthrough": fig_gpu_passthrough,
    "lvm-layers": fig_lvm,
    "nut-network": fig_nut,
    "prometheus-pull": fig_prometheus,
}

# Data-driven figures; see kb_figspecs.py.
from kb_figspecs import SPECS  # noqa: E402

LAYOUTS = {"flow": flow, "stack": stack, "steps": steps}
for _name, (_kind, _kw) in SPECS.items():
    FIGURES[_name] = lambda k=_kind, kw=_kw, n=_name: LAYOUTS[k](**kw, name=n)



def main(argv):
    OUT.mkdir(parents=True, exist_ok=True)
    only = argv[1:] or [""]
    for name, fn in FIGURES.items():
        if not any(name.startswith(o) for o in only):
            continue
        svg = fn()
        (OUT / f"{name}.svg").write_text(svg)
        print(f"{name}.svg  {len(svg.encode()) / 1024:.1f} KB")


if __name__ == "__main__":
    main(sys.argv)
