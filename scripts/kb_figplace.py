#!/usr/bin/env python3
"""Puts each figure listed in kb_figspecs.PLACE into its article: a
`![caption](fig:name)` line before the first `## ` heading, unless the article
already shows that figure. Safe to run again."""

from pathlib import Path

from kb_figspecs import PLACE

KB = Path(__file__).resolve().parent.parent / "docs" / "kb"

for article, (fig, caption) in PLACE.items():
    path = KB / f"{article}.md"
    lines = path.read_text().split("\n")
    if any(f"(fig:{fig})" in line for line in lines):
        continue
    at = next(i for i, line in enumerate(lines) if line.startswith("## "))
    lines[at:at] = [f"![{caption}](fig:{fig})", ""]
    path.write_text("\n".join(lines))
    print(f"{article}: {fig}")
