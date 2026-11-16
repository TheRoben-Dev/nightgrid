-- 0028 (M10w): the .env vault. One row per variable of a compose stack on a
-- host. `value` is the age ciphertext (base64) of the value, encrypted to the
-- vault key named by `key_id`; the key itself lives in the OS keychain
-- (rule 1). `masked` decides whether the UI hides the value until revealed. Part of profile
-- exports (the ciphertext; the key travels only with opted-in secrets).

CREATE TABLE env_vars (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    stack      TEXT NOT NULL,
    name       TEXT NOT NULL,
    value      TEXT NOT NULL,
    key_id     TEXT NOT NULL,
    masked     INTEGER NOT NULL DEFAULT 1 CHECK (masked IN (0, 1)),
    position   INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    UNIQUE (host_id, stack, name)
) STRICT;
