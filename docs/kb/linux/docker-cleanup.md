# Free disk space Docker is using

Images, stopped containers, build cache and container logs pile up until the root disk fills. Find what's big and clean it up without deleting anything you still need.

## What you end up with

- A clear picture of where Docker's space goes
- Safe cleanup commands, and which ones to avoid
- Limits so it doesn't come back

## Part 1 · Look first

```sh
docker system df
docker system df -v | head -40
sudo du -sh /var/lib/docker/* | sort -h
```

- [ ] **Images** "reclaimable" is images no container uses. **Build Cache** grows on hosts where you build images. **Local Volumes** reclaimable means volumes no container mounts: they may still hold data.

## Part 2 · Clean in safe order

- [ ] Stopped containers and unused networks:

```sh
docker container prune
docker network prune
```

- [ ] Images not used by any container, older than a week:

```sh
docker image prune -a --filter "until=168h"
```

- [ ] Build cache: `docker builder prune --filter "until=168h"`.
- [ ] **Volumes last, by name only.** `docker volume ls -f dangling=true`, check each with `docker volume inspect`, then `docker volume rm <name>`. Avoid `docker volume prune` and `docker system prune --volumes`: a stack that's down right now looks unused and loses its database.

## Part 3 · Logs and overlay leftovers

- [ ] Big container logs: see [Keep logs from filling the disk](kb:linux/logrotate) to cap them.
- [ ] Space Docker can't account for after a crash: stop Docker and check `/var/lib/docker/overlay2` against `docker system df`. Don't delete folders there by hand while Docker runs.

## Part 4 · Move Docker to a bigger disk

- [ ] Stop Docker, copy the data, point Docker at it:

```sh
sudo systemctl stop docker docker.socket
sudo rsync -aHAX /var/lib/docker/ /srv/docker/
echo '{ "data-root": "/srv/docker" }' | sudo tee /etc/docker/daemon.json
sudo systemctl start docker
```

- [ ] Merge `data-root` into an existing `daemon.json` instead of overwriting it. Remove the old folder only after everything runs.

#### Check

- [ ] `docker system df` shows the reclaimable amounts gone, and every stack in [Docker](view:docker) still starts.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** shows which mount is filling and how fast; a jump right after updates is usually old images.
- [Docker](view:docker) stack **Snapshots** record each service's exact image (one is taken before every update); rolling back to a pruned image pulls it again.
- Pruning on many hosts at once from [Automation](view:automation) asks you to confirm the targets by name.

## Limits and gotchas

- Rolling back to an image you pruned means pulling it again; tags that moved may not give you the same image.
- `docker system prune -a` also removes images for stacks that are stopped today but needed tomorrow.

## Related

- [Keep logs from filling the disk](kb:linux/logrotate)
- [Docker and compose basics](kb:basics/docker-compose)
- [Update containers without fear](kb:howto/container-updates)
