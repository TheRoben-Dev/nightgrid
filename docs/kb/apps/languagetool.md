# LanguageTool grammar checker

A self-hosted grammar, style and spell checker for 30+ languages, used by the LanguageTool browser extension, LibreOffice and many editors. Running your own means the text you type in web forms and emails never leaves your network.

## What you end up with

- The LanguageTool server in Docker, reachable on your LAN or tailnet
- The browser extension and LibreOffice using it instead of the cloud
- Optional n-gram data for better detection of commonly confused words

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `languagetool`:

```yaml
services:
  languagetool:
    image: erikvl87/languagetool:latest
    restart: unless-stopped
    environment:
      Java_Xms: 512m
      Java_Xmx: 1g
    ports:
      - "8010:8010"
```

- [ ] Test it in a [Terminal](view:terminal) tab: `curl -d "language=en-US" -d "text=This are a test." http://127.0.0.1:8010/v2/check` returns a JSON match for "This are".

## Part 2 · Clients

- [ ] **Browser extension**: settings → **Advanced settings** → **LanguageTool server** → **Other server**, `http://<host>:8010/v2`. Use the host's LAN or tailnet address.
- [ ] **LibreOffice**: **Tools → Options → Language Settings → LanguageTool Server**, enable it and enter the same base URL.
- [ ] Many editors (VS Code extensions, Obsidian plugins) accept a custom server URL the same way.

## Part 3 · n-grams (optional)

- [ ] Download the n-gram archive for your language from LanguageTool's site (English is about 8 GB unpacked), unpack it to `~/stacks/languagetool/ngrams/en`.
- [ ] Add `langtool_languageModel: /ngrams` to the environment and `./ngrams:/ngrams:ro` as a volume, raise `Java_Xmx` to `2g`, and redeploy.

#### Check

- [ ] Typing "their is a problem" in a web form shows the extension's underline, and the server's container log shows the check request.

## In Nightgrid

- **⧉** in [Docker](view:docker) isn't useful here (there's no web page); add `http://<host>:8010/v2/languages` to [Services](view:services) instead, which answers with the language list when it's healthy.
- The container's memory use is visible in [Docker](view:docker); it grows with the n-gram data.

## Limits and gotchas

- The free server has the open-source rules only; the paid cloud's "premium" style suggestions aren't included.
- Plain HTTP is fine on a LAN or tailnet. If you put it behind a public reverse proxy, add authentication there, the API has none.
- First checks after a start are slow while the JVM warms up.

## Related

- [Stirling-PDF toolkit](kb:apps/stirling-pdf)
- [Collabora Online for Nextcloud](kb:apps/collabora)
- [Prepare a Linux Docker host](kb:apps/docker-host)
