# JupyterLab notebooks on a server

Python notebooks running on a server with the RAM, cores or GPU your laptop lacks, opened from a browser. Good for data work, quick scripts against your homelab data, and learning.

## What you end up with

- JupyterLab on port 8888 behind a token or password
- Notebooks and data in a folder on the server
- Optional GPU access for PyTorch or similar

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `jupyter`:

```yaml
services:
  jupyter:
    image: quay.io/jupyter/scipy-notebook:latest
    restart: unless-stopped
    user: root
    environment:
      NB_UID: "1000"
      NB_GID: "1000"
      CHOWN_HOME: "yes"
    volumes:
      - ./work:/home/jovyan/work
    ports:
      - "127.0.0.1:8888:8888"
```

- [ ] Publishing on `127.0.0.1` keeps it off the LAN: notebooks run code as you, so treat the login like SSH.
- [ ] The login token is in the container log (`http://127.0.0.1:8888/lab?token=...`).
- [ ] Other images: `minimal-notebook` (small), `datascience-notebook` (R and Julia too), `pytorch-notebook` (add `-cuda12` tags and a GPU reservation for NVIDIA).

## Part 2 · Work

- [ ] Save notebooks under `work/`; everything else in the container is lost on update.
- [ ] Install extra packages in a notebook with `%pip install package`. For packages that should survive updates, build a small image `FROM` the one above.

#### Check

- [ ] A notebook cell `import pandas, sys; sys.version` runs, and the notebook file appears in `./work` on the host.

## In Nightgrid

- Open it with **⧉** on the container in [Docker](view:docker): ports on `127.0.0.1` go through an SSH tunnel automatically (see [Open app web UIs inside Nightgrid](kb:howto/web-uis)). Or use a saved tunnel in [Tunnels](view:tunnels).
- Long-running notebooks show as CPU or GPU load on [Overview](view:overview).

## Limits and gotchas

- Anyone with the token can run commands as the notebook user. Don't publish it on `0.0.0.0` without a reverse proxy login in front.
- Kernels keep their memory until stopped; shut down notebooks you're done with (Running tab).

## Related

- [Ollama and Open WebUI](kb:apps/ollama-openwebui)
- [Local AI GPU server](kb:guides/local-ai-gpu-server)
- [code-server (VS Code in the browser)](kb:apps/code-server)
