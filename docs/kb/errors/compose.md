# Errors: Docker Compose

Messages from `docker compose` (v2) and the old `docker-compose` (v1): YAML problems, variables, dependencies and project names. DRIFT's compose review checks a pasted file for common mistakes too.

### yaml: line <n>: did not find expected key
Means: The YAML is malformed near that line: wrong indentation, a missing colon, or a list item at the wrong level.
Fix: Use spaces only (no tabs), two per level, and check the lines just above. `docker compose config` validates the file.

### yaml: line <n>: mapping values are not allowed in this context
Means: A colon appears where YAML doesn't expect one, usually an unquoted value containing `: ` or wrong indentation.
Fix: Quote values with colons (`"10.0.0.1:8080:80"`), and check indentation.

### yaml: line <n>: found character that cannot start any token
Means: A tab or an odd character (like `@` or a backtick) starts a value.
Fix: Replace tabs with spaces; quote values that start with special characters.

### yaml: unmarshal errors:
Means: A key has the wrong type (a string where a list is expected, or the reverse).
Fix: The lines below name the key; compare with the compose reference (`ports` and `volumes` are lists).

### services.<name> Additional property <key> is not allowed
Means: A key isn't valid at that level: a typo (`enviroment`), or it's indented under the wrong parent.
Fix: Fix the spelling or indentation; `docker compose config` shows where.

### services.<name>.ports contains an invalid type, it should be a number, or an object
Means: A ports entry isn't a valid string or mapping, often unquoted `80:80` read as a base-60 number by old YAML parsers.
Fix: Always quote port mappings: `- "8080:80"`.

### services.<name>.environment must be a mapping
Means: The environment block mixes list (`- KEY=val`) and map (`KEY: val`) styles.
Fix: Pick one style for the whole block.

### no configuration file provided: not found
Means: `docker compose` found no compose.yaml/docker-compose.yml in this directory or its parents.
Fix: `cd` to the stack folder, or pass `-f <file>`.

### service "<name>" refers to undefined network <network>: invalid compose project
Means: A service lists a network that isn't defined in the top-level `networks:`.
Fix: Add it under `networks:` (with `external: true` if it exists already).

### service "<name>" refers to undefined volume <volume>: invalid compose project
Means: A named volume is used but not declared in the top-level `volumes:`.
Fix: Declare it, or use a path (`./data:/data`) for a bind mount.

### network <name> declared as external, but could not be found
Means: The stack expects an existing network (often a proxy network) that hasn't been created.
Fix: `docker network create <name>`, or start the stack that creates it first.

### volume "<name>" declared as external, but could not be found
Means: The stack expects an existing volume.
Fix: `docker volume create <name>`, or remove `external: true`.

### The "<var>" variable is not set. Defaulting to a blank string.
Means: The file uses `${VAR}` but it's not in the environment or the .env file next to the compose file.
Fix: Add it to .env (same folder as the compose file, or the one passed with `--env-file`), or give a default: `${VAR:-value}`. Nightgrid's .env vault keeps these per stack.

### required variable <var> is missing a value: <message>
Means: The file uses `${VAR:?message}` and the variable isn't set.
Fix: Set it in .env.

### invalid interpolation format for services.<name>.<key>: "<value>"
Means: A literal `$` in a value (often a password or a bcrypt hash) is read as a variable.
Fix: Escape it as `$$`, or move the value into an env_file, which isn't interpolated.

### Couldn't find env file: <path>
Means: An `env_file:` entry points at a file that doesn't exist.
Fix: Create it, or mark it optional (`env_file: [{path: ./x.env, required: false}]` on Compose 2.24+).

### dependency failed to start: container <name> is unhealthy
Means: A service with `depends_on: condition: service_healthy` waited for a dependency whose healthcheck failed.
Fix: Fix the dependency first: `docker inspect -f '{{json .State.Health}}' <name>`.

### dependency failed to start: container <name> exited (<n>)
Means: A dependency with `condition: service_completed_successfully` (or a started one) exited with an error.
Fix: `docker compose logs <name>`.

### service "<name>" depends on undefined service "<dep>": invalid compose project
Means: depends_on names a service that isn't in the file (typo, or it lives in another stack).
Fix: Correct the name; cross-stack dependencies can't be expressed with depends_on.

### Error response from daemon: Conflict. The container name "/<name>" is already in use by container "<id>"
Means: A `container_name:` collides with an existing container, often left from the same stack run under another project name.
Fix: `docker rm` the old container (after checking), or drop `container_name` and let compose name it.

### WARN[0000] Found orphan containers ([<names>]) for this project. If you removed or renamed this service in your compose file, you can run this command with the --remove-orphans flag to clean it up.
Means: Containers from services no longer in the file still belong to the project.
Fix: `docker compose up -d --remove-orphans` once you're sure they're unneeded.

### WARN[0000] <file>: the attribute `version` is obsolete, it will be ignored, please remove it to avoid potential confusion
Means: Compose v2 ignores the top-level `version:` key.
Fix: Delete the `version:` line.

### docker-compose: command not found
Means: The old standalone Compose v1 isn't installed; modern Docker ships the plugin as `docker compose` (with a space).
Fix: Use `docker compose`. Install the plugin package (`docker-compose-plugin`) if `docker compose version` fails.

### docker: 'compose' is not a docker command.
Means: The Compose v2 plugin isn't installed.
Fix: Install `docker-compose-plugin` from Docker's repo (or your distro's `docker-compose-v2`).

### ERROR: Version in "./docker-compose.yml" is unsupported.
Means: The old docker-compose v1 doesn't understand a newer file format.
Fix: Use `docker compose` (v2).

### ERROR: for <name>  Cannot start service <name>: <details>
Means: docker-compose v1's wording when a container failed to start; the details follow.
Fix: Read the details (port in use, mount error); move to Compose v2.

### Error response from daemon: invalid mount config for type "volume": invalid mount path: '<path>' mount path must be absolute
Means: The container side of a volume isn't an absolute path.
Fix: Start the container path with `/`.

### services.<name>.deploy.resources.reservations.devices.capabilities is required
Means: A GPU reservation block is missing `capabilities: [gpu]`.
Fix: Add it under the device entry.

### Error: failed to bind host port for 0.0.0.0:<port>:<ip>:<port>/tcp: address already in use
Means: Compose couldn't publish a port because the host or another container uses it.
Fix: Find the user (`ss -ltnp`, `docker ps`) and change the host port.

### unable to prepare context: path "<path>" not found
Means: A `build:` context path doesn't exist.
Fix: Paths are relative to the compose file; check the folder.

### pull access denied for <image>, repository does not exist or may require 'docker login'
Means: compose tried to pull an image that's private or doesn't exist. When `build:` is set, the image may simply not be built yet.
Fix: Check the name; run `docker compose build` first, or `docker login`.

### Error response from daemon: error while mounting volume '<path>': failed to mount local volume: mount :<path>:<path>, flags: 0x1000: no such file or directory
Means: A named volume with a bind `device:` path points at a host path that doesn't exist.
Fix: Create the host directory first.

### Error response from daemon: error while mounting volume '<path>': failed to mount local volume: mount <src>:<dst>: permission denied
Means: An NFS or CIFS volume (driver_opts) was refused by the server, or options are wrong.
Fix: Test the same mount by hand on the host; check the export allows the host and the options (`addr=`, `nfsvers=`).

### no such service: <name>
Means: The command named a service that isn't in this project.
Fix: `docker compose config --services` lists them.

### service "<name>" has neither an image nor a build context specified: invalid compose project
Means: A service lacks both `image:` and `build:`.
Fix: Add one of them; check indentation didn't move them under another key.

### Error response from daemon: Get "<url>": unauthorized: authentication required
Means: Pulling from a registry that needs login (ghcr.io private packages, a private registry).
Fix: `docker login <registry>` with a token that can read packages.

### Error response from daemon: endpoint with name <name> already exists in network <network>
Means: A stale endpoint from an old container is stuck on that network.
Fix: `docker network disconnect -f <network> <name>`; restart Docker if it persists.

### Container <name>  Error response from daemon: Cannot restart container <name>: <details>
Means: A restart failed; the details give the reason (port, mount, image).
Fix: Read the detail and `docker compose logs <name>`.

### the input device is not a TTY
Means: `docker compose exec` or `run` with a terminal was used from a script or cron.
Fix: Add `-T` to disable the TTY in non-interactive use.

### WARN[0000] networks.default: external.name is deprecated. Please set name and external: true
Means: Old syntax for external networks.
Fix: Use `name: <net>` with `external: true`.
