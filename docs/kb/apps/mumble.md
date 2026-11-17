# Mumble voice chat

Low-latency voice chat you host yourself, for gaming nights or a group of friends: channels, push-to-talk, positional audio for some games, and clients for every platform.

## What you end up with

- A Mumble server on port 64738 (TCP and UDP)
- Registered users and channels with permissions
- No accounts at a third party

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `mumble`:

```yaml
services:
  mumble:
    image: mumblevoip/mumble-server:latest
    restart: unless-stopped
    ports:
      - "64738:64738"
      - "64738:64738/udp"
    environment:
      MUMBLE_SUPERUSER_PASSWORD: ${SUPERUSER_PASSWORD}
      MUMBLE_CONFIG_SERVER_PASSWORD: ${SERVER_PASSWORD}
      MUMBLE_CONFIG_USERS: "50"
    volumes:
      - ./data:/data
```

- [ ] `MUMBLE_CONFIG_*` variables set any `mumble-server.ini` option; `SERVER_PASSWORD` is what friends type to join.

## Part 2 · Admin

- [ ] Connect with the Mumble client as user `SuperUser` and the superuser password.
- [ ] Create channels (right-click → Add), then register your own normal user and give it admin through the root channel's ACL (Groups → admin → add).
- [ ] Log in as your normal user from then on.

## Part 3 · Friends

- [ ] Forward TCP and UDP 64738, or connect over your tailnet.
- [ ] Clients run the audio wizard once; push-to-talk keeps background noise out.

#### Check

- [ ] Two clients in the same channel hear each other with no noticeable delay.

## In Nightgrid

- The container log in [Docker](view:docker) lists connections and rejected passwords.
- A voice server uses very little CPU; it fits next to game servers on the same host.

## Limits and gotchas

- UDP must reach the server; with only TCP, audio falls back to a slower tunnel through it.
- The server certificate is self-signed by default; clients warn once and remember it.

## Related

- [Game server host](kb:guides/game-server-host)
- [Palworld dedicated server](kb:apps/palworld)
- [Reach your servers away from home](kb:howto/remote-access)
