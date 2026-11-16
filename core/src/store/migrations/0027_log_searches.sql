-- 0027 (M10u): saved cross-host log searches. `query` is the JSON of a
-- LogSearchQuery (text, regex, hosts, sources, filters, look-back). `alert`
-- raises an alert when new lines match. Part of profile exports.

CREATE TABLE log_searches (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL,
    query TEXT NOT NULL,
    alert INTEGER NOT NULL DEFAULT 0 CHECK (alert IN (0, 1))
) STRICT;
