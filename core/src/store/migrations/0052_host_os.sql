-- 0049 (1.8): which family of OS a host runs, learned from its SSH server's
-- version line on connect: '' (not known yet), 'posix' or 'windows'
-- (OpenSSH for Windows). Views show what works on Windows hosts and say why
-- the rest can't, without connecting first.

ALTER TABLE hosts ADD COLUMN os TEXT NOT NULL DEFAULT '';
