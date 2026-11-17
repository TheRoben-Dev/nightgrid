# Excalidraw whiteboard

A hand-drawn style whiteboard for network diagrams, rack layouts and quick sketches, running entirely in the browser. Self-hosting keeps the app on your server; drawings stay in your browser or in files you save.

## What you end up with

- The Excalidraw editor on your LAN
- Diagrams saved as `.excalidraw` files (or PNG/SVG exports) you keep with your docs

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `excalidraw`:

```yaml
services:
  excalidraw:
    image: excalidraw/excalidraw:latest
    restart: unless-stopped
    ports:
      - "3030:80"
```

- [ ] Open port 3030. There's no login and nothing stored on the server.

## Part 2 · Use it

- [ ] Draw, then menu → **Save to** a `.excalidraw` file, or **Export image** to PNG or SVG (tick "Embed scene" to reopen and edit the PNG later).
- [ ] Libraries (the book icon) add ready-made shapes: servers, network gear, cloud icons.

#### Check

- [ ] Save a drawing, reload the page, open the file again: it's all there.

## In Nightgrid

- Upload exported diagrams next to a host's docs with [Files](view:files); the [Network](view:network) map gives you the live picture to draw from.
- The self-hosted image doesn't include live collaboration; that needs Excalidraw's separate room server.

## Limits and gotchas

- Without saving a file, a drawing lives only in that browser's local storage.
- Library items you browse online come from excalidraw.com; the editor itself works offline once loaded.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [IT-Tools toolbox](kb:apps/it-tools)
- [NetBox network documentation](kb:apps/netbox)
