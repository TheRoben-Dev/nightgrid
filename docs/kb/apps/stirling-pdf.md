# Stirling-PDF toolkit

Merge, split, compress, OCR, sign and convert PDFs in the browser, on your own server, so private documents never go to an online converter.

## What you end up with

- A PDF toolbox at `pdf.example.com` for the whole household

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `stirling-pdf`:

```yaml
services:
  stirling-pdf:
    image: stirlingtools/stirling-pdf:latest
    restart: unless-stopped
    environment:
      DOCKER_ENABLE_SECURITY: "false"
    volumes:
      - ./trainingData:/usr/share/tessdata
      - ./configs:/configs
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `pdf.example.com` to `stirling-pdf:8080`.
- [ ] OCR in other languages: add the tesseract language files to `./trainingData`.
- [ ] Logins: set `DOCKER_ENABLE_SECURITY: "true"` (and the security settings from its docs) if anyone outside your household can reach it.

#### Check

- [ ] Merge two PDFs and download the result.

## Limits and gotchas

- The full image is large (over 1 GB) because it bundles LibreOffice and OCR; there are smaller variants without them.
- Processing big PDFs uses a lot of RAM briefly; give the host headroom.

## Related

- [Paperless-ngx documents](kb:apps/paperless-ngx)
