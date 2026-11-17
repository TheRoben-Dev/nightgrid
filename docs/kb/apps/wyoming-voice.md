# Local voice assistant for Home Assistant

Speech-to-text (Whisper), text-to-speech (Piper) and wake word detection (openWakeWord) on your own server, connected to Home Assistant's Assist over the Wyoming protocol. "Turn off the kitchen lights" never leaves the house.

## What you end up with

- Three small services Home Assistant uses for voice
- Assist working from the HA app, a browser, or voice satellites
- No cloud speech service

## Before you start

- Home Assistant running (Docker or HA OS) on the same LAN
- A host with a decent CPU, or an NVIDIA GPU for faster Whisper models

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `voice`:

```yaml
services:
  whisper:
    image: rhasspy/wyoming-whisper:latest
    restart: unless-stopped
    command: --model base-int8 --language en
    volumes:
      - ./whisper:/data
    ports:
      - "10300:10300"
  piper:
    image: rhasspy/wyoming-piper:latest
    restart: unless-stopped
    command: --voice en_US-lessac-medium
    volumes:
      - ./piper:/data
    ports:
      - "10200:10200"
  openwakeword:
    image: rhasspy/wyoming-openwakeword:latest
    restart: unless-stopped
    command: --preload-model ok_nabu
    ports:
      - "10400:10400"
```

- [ ] Change `--language` and pick a Piper voice in your language. Models download on first start into the data folders.
- [ ] Bigger Whisper models (`small-int8`, `medium-int8`) understand better but answer slower on CPU.

## Part 2 · Connect Home Assistant

- [ ] Settings → Devices & services → Add integration → **Wyoming Protocol**, three times: host and port 10300, 10200 and 10400.
- [ ] Settings → Voice assistants → your assistant: speech-to-text **faster-whisper**, text-to-speech **piper**, wake word **openwakeword**.

#### Check

- [ ] In the HA app, tap Assist, hold the mic and say "what time is it": the reply is spoken back.

## In Nightgrid

- Watch the first transcriptions' CPU spike on [Overview](view:overview) to pick the right Whisper model size for the host.
- The three containers' logs in [Docker](view:docker) show each request, which helps when Assist "didn't hear" something.

## Limits and gotchas

- Assist understands sentences about your entities and areas, not open questions; add an LLM conversation agent (for example via Ollama) if you want chat.
- Expose only the entities you want controllable by voice (Settings → Voice assistants → Expose).

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Ollama and Open WebUI](kb:apps/ollama-openwebui)
- [Smart home hub](kb:guides/smart-home-hub)
