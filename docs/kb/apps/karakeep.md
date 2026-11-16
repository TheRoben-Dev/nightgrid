# Karakeep bookmarks

Save links, notes and images with full-page archives and search, from browser extensions and phone apps. (Formerly Hoarder.)

## Part 1 · The stack

- [ ] Download Karakeep's `docker-compose.yml` for your version from its docs into `/opt/stacks/karakeep`. It runs the web app, a headless Chrome for archiving and Meilisearch.
- [ ] `.env` (or the stack's **Vault**):

```sh
KARAKEEP_VERSION=release
NEXTAUTH_SECRET=<openssl rand -base64 36>
MEILI_MASTER_KEY=<openssl rand -base64 36>
NEXTAUTH_URL=https://links.example.com
```

- [ ] Bring it up in [Docker](view:docker) and route `links.example.com` to the web container's port 3000.
- [ ] Sign up the first user (becomes admin), then set `DISABLE_SIGNUPS=true`.

## Part 2 · Optional AI tagging

- [ ] Point `OLLAMA_BASE_URL` at your Ollama server ([Ollama and Open WebUI](kb:apps/ollama-openwebui)) with `INFERENCE_TEXT_MODEL` set, and links get tags automatically, locally.

#### Check

- [ ] The browser extension saves a page and its archive opens offline.

## Limits and gotchas

- The headless browser uses a few hundred MB of RAM; archive-heavy use grows `./data` steadily.

## Related

- [Ollama and Open WebUI](kb:apps/ollama-openwebui)
- [FreshRSS feed reader](kb:apps/freshrss)
