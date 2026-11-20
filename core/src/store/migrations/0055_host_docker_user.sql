-- 0055: a host's Docker user. When set, Nightgrid keeps logging in as the
-- host's SSH user and runs docker as this account through `sudo -n -u`
-- (a dedicated docker account with its own UID/GID, e.g. on a NAS).
-- NULL = docker runs as the SSH login, as before.

ALTER TABLE hosts ADD COLUMN docker_user TEXT;
