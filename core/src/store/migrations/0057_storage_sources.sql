-- 0057 (D-200): where each mount's space comes from, so the forecast can
-- tell a host's own disks from NFS/SMB shares, Windows drives passed into
-- WSL and datasets of one pool, and count shared storage once.
-- Rows recorded before this have '' and are treated as local disks until
-- the next sample fills them in.

ALTER TABLE disk_usage ADD COLUMN device TEXT NOT NULL DEFAULT '';
ALTER TABLE disk_usage ADD COLUMN fstype TEXT NOT NULL DEFAULT '';

-- "This host's disks are stored on that host's drive": a VM whose disk
-- images live on a physical drive of its hypervisor (or a Windows PC's
-- H:). Kept apart from alert_deps on purpose: a share mounted from a NAS
-- is not a dependency of the host's own storage, but a VM's disk file is.
-- Config, so part of the export.
CREATE TABLE storage_links (
    id            INTEGER PRIMARY KEY,
    host_id       INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    owner_host_id INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    owner_mount   TEXT    NOT NULL,
    CHECK (host_id != owner_host_id),
    UNIQUE (host_id, owner_host_id, owner_mount)
) STRICT;
