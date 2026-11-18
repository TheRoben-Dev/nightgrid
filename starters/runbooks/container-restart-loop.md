---
id: container-restart-loop
title: "Runbook: container down or restarting"
summary: Read why a container keeps stopping and get it running again.
alertKinds: [containerDown]
tags: [runbook, starter, docker]
library: [errors/docker, errors/compose, linux/memory-limits]
---
A container stopped, or keeps restarting (a crash loop). The answer is almost always in its last log lines or its exit code.

- [ ] Open the container in [Docker](view:docker) and read the last lines of its log. Or in a terminal (replace `NAME`):

```sh
docker logs --tail 80 NAME
```

- [ ] Read the exit code and whether the kernel killed it for memory:

```sh
docker inspect --format 'exit={{.State.ExitCode}} oom={{.State.OOMKilled}} restarts={{.RestartCount}} error={{.State.Error}}' NAME
```

- [ ] Exit code 137 or `oom=true`: it ran out of memory. Raise its limit or the host's memory, or find the leak.
- [ ] Exit code 1 right after start: usually config. Look for a missing variable, a wrong path or a port already taken. Check the stack's `.env` in the vault.
- [ ] "permission denied" on a volume: the folder's owner doesn't match the user the container runs as (`PUID`/`PGID`).
- [ ] Changed nothing and it broke after an update? Roll back from the stack's snapshot in Docker → STACKS.
- [ ] Once fixed, start it again:

```sh
docker compose up -d
```

More in [Docker errors](kb:errors/docker) and [Compose errors](kb:errors/compose).
