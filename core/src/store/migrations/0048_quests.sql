-- 0048 (1.4): the quest log. A Library guide the user started as a quest,
-- on one host (or none, for guides with nothing to check), with the steps
-- done so far as Markdown block-item keys ("12-0"). Config, not history:
-- quests travel in the profile export.

CREATE TABLE quests (
    id           INTEGER PRIMARY KEY,
    article_id   TEXT NOT NULL,
    host_id      INTEGER REFERENCES hosts(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    -- JSON array of done step keys.
    ticks        TEXT NOT NULL DEFAULT '[]',
    started_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    completed_at INTEGER
) STRICT;

CREATE INDEX quests_article ON quests(article_id, host_id);
