# Speedtest Tracker

Measure your internet connection on a schedule and keep the history, so you can show your ISP the evenings it drops to a fraction of what you pay for.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `speedtest`:

```yaml
services:
  speedtest-tracker:
    image: lscr.io/linuxserver/speedtest-tracker:latest
    restart: unless-stopped
    environment:
      PUID: 1000
      PGID: 1000
      APP_KEY: ${APP_KEY}
      APP_URL: https://speed.example.com
      DB_CONNECTION: sqlite
      SPEEDTEST_SCHEDULE: "15 */2 * * *"
    volumes:
      - ./config:/config
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Generate `APP_KEY` as the image's docs describe (`echo -n 'base64:'; openssl rand -base64 32`) and put it in the **Vault**.
- [ ] Route `speed.example.com` to port 80 of the container and log in with the default admin from the docs; change it.

#### Check

- [ ] Results appear every two hours with download, upload and ping.

## Limits and gotchas

- Each test uses real bandwidth; every two hours is plenty.
- For speeds between your own machines, use Nightgrid's [Find slow links with iperf3](kb:howto/speed-test).

## Related

- [Find slow links with iperf3](kb:howto/speed-test)
