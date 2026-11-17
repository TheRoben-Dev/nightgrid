#!/usr/bin/env python3
"""Keeps the catalog's error-reference entries in step with docs/kb/errors.

Each docs/kb/errors/<area>.md becomes one catalog line between the
"Error reference" comment and "// errors:end" in ui/src/lib/kb/catalog.ts,
with the topic from CAT below (default "linux"). Run it after adding a file.
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
CAT = {
    "docker": "docker", "compose": "docker", "podman": "docker",
    "ssh": "security", "tls": "security", "users": "security", "firewall": "security",
    "network": "network", "dns": "network", "web": "network",
    "storage": "storage", "disks": "storage", "nfs-smb": "storage", "backup": "storage",
    "databases": "dev", "git": "dev", "runtimes": "dev",
    "virtualization": "virtualization", "kernel": "hardware",
}

lines = []
for f in sorted((ROOT / "docs/kb/errors").glob("*.md")):
    if f.stem.lower() == "readme":
        continue
    title = re.search(r"^#\s+(.+)$", f.read_text(), re.M).group(1).strip()
    lines.append(f'  {{ id: "errors/{f.stem}", shelf: "errors", title: {title!r}, cat: "{CAT.get(f.stem, "linux")}" }},'.replace("'", '"'))

p = ROOT / "ui/src/lib/kb/catalog.ts"
s = p.read_text()
start = s.index("  // Error reference (generated")
start = s.index("\n", start) + 1
end = s.index("  // errors:end")
p.write_text(s[:start] + "\n".join(lines) + "\n" + s[end:])
print(f"{len(lines)} error articles")
