# Knowledge base guides

Guide articles for the Library (Knowledge → Library). They are
bundled with the app and indexed for DRIFT. Keep them free of
real hosts, addresses and secrets: use placeholders (`<ct-ip>`, `example.com`,
`192.168.1.0/24`). Links like `[Docker](view:docker)` open a view; keep the
UI names in sync with the app.

Pictures: put `![caption](fig:name)` on a line of its own; `name` is an SVG
in `docs/kb/figures`, drawn by `scripts/kb_figures.py` (run it after editing).
A figure on the first line under the title is the article's briefing banner.
Most diagrams are data in `scripts/kb_figspecs.py` (flow, stack or steps
layouts; the script warns when a label won't fit). List the article in
`PLACE` there and run `scripts/kb_figplace.py` to insert the figure line.
