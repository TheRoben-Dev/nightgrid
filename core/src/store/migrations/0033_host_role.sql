-- 0033: a host's role. "client" marks a machine that isn't a server (a laptop,
-- a handheld, a desktop): views never pick one by default.

ALTER TABLE hosts ADD COLUMN role TEXT NOT NULL DEFAULT 'server';
