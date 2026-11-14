-- 0002: SSH connection details for hosts.
-- auth_method: 'agent' (default, preferred), 'key' (identity file), 'password'.
-- remember_secret: opt-in flag to keep the password / key passphrase in the OS
-- keychain. The secret itself is never stored here.

ALTER TABLE hosts ADD COLUMN auth_method TEXT NOT NULL DEFAULT 'agent'
    CHECK (auth_method IN ('agent', 'key', 'password'));
ALTER TABLE hosts ADD COLUMN identity_file TEXT;
ALTER TABLE hosts ADD COLUMN proxy_jump TEXT;
ALTER TABLE hosts ADD COLUMN remember_secret INTEGER NOT NULL DEFAULT 0 CHECK (remember_secret IN (0, 1));
-- Alias of the ~/.ssh/config `Host` entry this host was imported from, if any.
ALTER TABLE hosts ADD COLUMN ssh_alias TEXT;

CREATE UNIQUE INDEX hosts_ssh_alias_idx ON hosts(ssh_alias) WHERE ssh_alias IS NOT NULL;
