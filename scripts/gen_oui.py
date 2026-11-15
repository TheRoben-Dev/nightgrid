#!/usr/bin/env python3
"""Builds core/data/oui.tsv (MAC prefix -> short vendor name) from the IEEE
MA-L registry as packaged by the `oui-data` npm package (BSD-2-Clause):

    npm pack oui-data && tar xzf oui-data-*.tgz
    python3 -I scripts/gen_oui.py package/index.json > core/data/oui.tsv
"""
import json, re, sys

SUFFIX = re.compile(
    r"[,.]?\s+(inc|incorporated|corp|corporation|co|company|ltd|limited|llc|l\.l\.c|gmbh|ag|sa|s\.a|srl|bv|b\.v|nv|ab|as|oy|plc|pty|pte|kg|spa|s\.p\.a|kk|k\.k|technologies|technology|tech|electronics|international|holdings|group)\.?$",
    re.I,
)

def short(name: str) -> str:
    n = name.split("\n")[0].strip().rstrip(".,")
    n = re.sub(r"[\s,]*co\.?,?\s*ltd\.?$", "", n, flags=re.I)
    for _ in range(4):
        m = SUFFIX.sub("", n).strip().rstrip(".,")
        if m == n or not m:
            break
        n = m
    if n.isupper() and len(n) > 4:
        n = " ".join(w if len(w) <= 3 else w.capitalize() for w in n.split())
    return n.replace("\t", " ")[:40]

data = json.load(open(sys.argv[1], encoding="utf-8"))
for k in sorted(data):
    if len(k) == 6 and all(c in "0123456789ABCDEFabcdef" for c in k):
        print(f"{k.upper()}\t{short(data[k])}")
