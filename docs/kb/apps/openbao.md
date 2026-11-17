# OpenBao secrets manager

A server for passwords, API keys and certificates that scripts and apps fetch at runtime, with access policies and an audit trail. OpenBao is the open-source fork of HashiCorp Vault and uses the same API and concepts. Worth it once several machines and scripts share secrets; for one person's logins, a password manager is simpler.

## What you end up with

- A single OpenBao server with integrated (Raft) storage
- It initialised and unsealed, with the unseal keys and root token stored safely
- A key/value secrets engine, a policy and a token a script can use

## Part 1 · Config and stack

- [ ] In [Files](view:files) create `~/stacks/openbao/config/bao.hcl`:

```hcl
ui = true
disable_mlock = true

storage "raft" {
  path    = "/openbao/data"
  node_id = "bao1"
}

listener "tcp" {
  address     = "0.0.0.0:8200"
  tls_disable = 1
}

api_addr     = "http://127.0.0.1:8200"
cluster_addr = "http://127.0.0.1:8201"
```

- [ ] [Docker](view:docker) → **+ New stack** named `openbao`:

```yaml
services:
  openbao:
    image: openbao/openbao:latest
    restart: unless-stopped
    command: server
    environment:
      BAO_ADDR: http://127.0.0.1:8200
    volumes:
      - ./config:/openbao/config:ro
      - ./data:/openbao/data
    ports:
      - "127.0.0.1:8200:8200"
```

- [ ] `tls_disable` is only acceptable because the port is bound to `127.0.0.1` and HTTPS comes from your reverse proxy. Never expose 8200 without TLS.
- [ ] `server` loads every `.hcl` file in `/openbao/config`. The `data` folder must be writable by the container's user; if the log shows permission denied on `/openbao/data`, make it owned by that user (`sudo chown -R 100:1000 data` for the official image) and redeploy.

## Part 2 · Initialise and unseal

- [ ] In a [Terminal](view:terminal) tab on the host: `docker exec -it openbao-openbao-1 bao operator init` (`BAO_ADDR` in the stack points the CLI at the server).
- [ ] It prints five unseal keys and a root token **once**. Store them in your password manager, split up if you like; without three of the keys the data can't be opened again.
- [ ] Unseal: run `bao operator unseal` three times with three different keys (same `docker exec` prefix). The UI at the proxied address now accepts the root token.

## Part 3 · Use it

- [ ] Log in with the root token in the CLI (`bao login`), enable a store: `bao secrets enable -path=kv kv-v2`, and add a secret: `bao kv put kv/backup restic_password=...`.
- [ ] Write a policy file that allows `read` on `kv/data/backup`, load it with `bao policy write backup backup.hcl`, then `bao token create -policy=backup -period=768h` for the script.
- [ ] The script fetches with `BAO_ADDR=https://bao.example.com BAO_TOKEN=... bao kv get -field=restic_password kv/backup`, or with curl against `/v1/kv/data/backup`.
- [ ] Revoke the root token when setup is done (`bao token revoke <root>`); create a new one later with the unseal keys if needed.

#### Check

- [ ] Restart the stack: OpenBao comes up **sealed**, the script fails, and after unsealing it works again. That is the normal behaviour, not a fault.

## In Nightgrid

- Add `https://bao.example.com/v1/sys/health` to [Services](view:services): it answers 200 when unsealed and 503 when sealed, which fails the default `200-399` check, so a reboot that left it sealed alerts you.
- For a single stack's passwords, the stack's own **Vault** in [Docker](view:docker) is simpler and needs no unsealing.

## Limits and gotchas

- Every restart needs a manual unseal unless you set up auto-unseal (a cloud KMS or a second transit server). That is the price of keys never being stored next to the data.
- Back up with `bao operator raft snapshot save` from a token with permission; copying the `data` folder of a running server is not safe.
- Commands are `bao` where Vault docs say `vault`; most Vault guides apply as written otherwise.

## Related

- [Vaultwarden password manager](kb:apps/vaultwarden)
- [Private certificate authority with step-ca](kb:apps/step-ca)
- [Write your own systemd service](kb:linux/systemd-units)
