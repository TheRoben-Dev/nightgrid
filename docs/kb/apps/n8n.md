# n8n workflow automation

Connect apps and APIs with visual workflows: "when a webhook fires, post to ntfy", "every morning, summarise new RSS items", "when a file lands in a folder, process it".

![A workflow from trigger to action.](fig:app-flows)

## What you end up with

- n8n at `n8n.example.com` with credentials encrypted by your own key
- Webhooks reachable by the services that call them

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `n8n`:

```yaml
services:
  n8n:
    image: docker.n8n.io/n8nio/n8n:latest
    restart: unless-stopped
    environment:
      N8N_HOST: n8n.example.com
      N8N_PROTOCOL: https
      WEBHOOK_URL: https://n8n.example.com/
      N8N_ENCRYPTION_KEY: ${N8N_ENCRYPTION_KEY}
      GENERIC_TIMEZONE: Etc/UTC
    volumes:
      - ./data:/home/node/.n8n
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] `N8N_ENCRYPTION_KEY` (random, 32+ characters) goes in the **Vault**. Lose it and stored credentials can't be decrypted.
- [ ] `./data` must be writable by uid 1000: `sudo chown -R 1000:1000 data`.
- [ ] Route `n8n.example.com` to `n8n:5678` and create the owner account.

## Part 2 · Ideas with Nightgrid

- [ ] Nightgrid's **JSON POST** alert webhook (Settings → Alerts) can target an n8n Webhook node: route alerts to email, chat or a ticket system n8n supports.
- [ ] Use **Send test** in Settings → Alerts to see the payload in n8n's execution log.

#### Check

- [ ] A test workflow with a Webhook trigger runs when called.

## Limits and gotchas

- Workflows that hold API keys make n8n a high-value target. Keep the editor behind a login or a tailnet; expose only `/webhook/` paths if something outside must call it.
- SQLite is fine for light use; switch to PostgreSQL for many executions.

## Related

- [Tune alerts so only real problems notify](kb:howto/alerts)
- [ntfy push notifications](kb:apps/ntfy)
