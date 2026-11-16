# Ollama and Open WebUI

Run local language models with Ollama and chat with them in Open WebUI, a ChatGPT-style interface with accounts, document chat and model management. Nothing leaves your network.

## What you end up with

- Ollama serving models on your GPU (or CPU)
- Open WebUI at `ai.example.com` for the household
- GPU load and VRAM visible in Nightgrid's host monitor

## Before you start

- An NVIDIA GPU with the driver and NVIDIA Container Toolkit installed, an AMD GPU with ROCm support, or patience on CPU.
- VRAM sets model size: 8 GB runs 7–8B models at 4-bit, 24 GB runs ~30B. The full build, with power and remote access, is in [Local AI GPU server](kb:guides/local-ai-gpu-server).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `ai`:

```yaml
services:
  ollama:
    image: ollama/ollama:latest
    restart: unless-stopped
    volumes:
      - ./ollama:/root/.ollama
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
  open-webui:
    image: ghcr.io/open-webui/open-webui:main
    restart: unless-stopped
    depends_on: [ollama]
    environment:
      OLLAMA_BASE_URL: http://ollama:11434
      WEBUI_SECRET_KEY: ${WEBUI_SECRET_KEY}
    volumes:
      - ./open-webui:/app/backend/data
    networks: [default, proxy]
networks:
  proxy:
    external: true
```

- [ ] AMD: use `ollama/ollama:rocm` and pass `/dev/kfd` and `/dev/dri` as devices instead of the `deploy` block.
- [ ] Route `ai.example.com` to `open-webui:8080`. The first account created is the admin; turn off sign-ups afterwards in the admin settings.

## Part 2 · Models

- [ ] Pull a model from a [Terminal](view:terminal):

```sh
docker exec -it ai-ollama-1 ollama pull llama3.1:8b
docker exec -it ai-ollama-1 ollama list
```

- [ ] Or pull from Open WebUI's admin settings → Models.

## Part 3 · In Nightgrid

- [ ] The host monitor shows GPU busy %, VRAM, temperature and power (NVIDIA via `nvidia-smi`, AMD via amdgpu), with history charts.
- [ ] A command widget for loaded models: `docker exec ai-ollama-1 ollama ps | tail -n +2 | wc -l`.

#### Check

- [ ] A chat answer streams in Open WebUI, and the GPU chart jumps while it does.

## Limits and gotchas

- Models are large (4–40 GB each); keep `./ollama` on fast local storage and watch the disk forecast.
- Don't publish port 11434: Ollama's API has no authentication.
- Nightgrid's own DRIFT helper runs its model locally inside the app and doesn't use this server.

## Related

- [Local AI GPU server](kb:guides/local-ai-gpu-server)
- [Get the most out of DRIFT](kb:howto/drift)
- [Uptime history and monthly reports](kb:howto/uptime-reports)
