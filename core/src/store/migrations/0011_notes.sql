-- 0011: notes and runbooks (M10e). Markdown text, optionally tied to a host.
-- Text that looks like it carries a secret is refused on save (see
-- core/src/redact.rs), so nothing here needs encrypting.

CREATE TABLE notes (
    id          INTEGER PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    host_id     INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
    -- JSON array of lower-case tags.
    tags        TEXT NOT NULL DEFAULT '[]',
    pinned      INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX notes_host ON notes(host_id);
