# IT-Tools toolbox

A web page full of small tools you'd otherwise search for: subnet calculator, chmod calculator, JWT and base64 decoders, hash and UUID generators, cron explainer, QR codes, MAC lookup and dozens more. Everything runs in the browser, nothing leaves it.

## What you end up with

- One bookmark for the conversions and calculators you use at the bench and in the terminal
- Works offline once loaded

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `it-tools`:

```yaml
services:
  it-tools:
    image: corentinth/it-tools:latest
    restart: unless-stopped
    ports:
      - "8090:80"
```

- [ ] Open port 8090 and star the tools you use in the sidebar.

## Part 2 · Handy picks

- [ ] **IPv4 subnet calculator** and **IPv4 range expander** for planning VLANs.
- [ ] **Chmod calculator**, **Crontab generator**, **Docker run to compose**.
- [ ] **JWT parser**, **Base64**, **URL encode**, **Hash text**.
- [ ] **MAC address lookup** and **MAC generator**.

#### Check

- [ ] Paste a `docker run` command into the converter and get a compose file back.

## In Nightgrid

- Some of these are built in: DRIFT explains cron lines and regexes, and [Network](view:network) → **Tools** has **Subnet** and **MAC lookup** (offline vendor table).
- It's a static page: no data, nothing to back up.

## Limits and gotchas

- Static and harmless, but there's no login; keep it on the LAN or tailnet anyway.

## Related

- [Diagnose network problems with Network → Tools](kb:howto/network-tools)
- [Get the most out of DRIFT](kb:howto/drift)
- [Stirling-PDF toolkit](kb:apps/stirling-pdf)
