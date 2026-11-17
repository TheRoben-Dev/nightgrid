# Errors: NFS, SMB and network shares

Messages when mounting or serving NFS and SMB/CIFS shares, and from sshfs and rclone mounts. See also [Mount an SMB share](kb:linux/mount-smb).

### mount.nfs: access denied by server while mounting <server>:<path>
Means: The NFS server refused this client: the export doesn't list its IP/subnet, or the path differs from the export.
Fix: On the server: `exportfs -v`. Add the client's subnet to /etc/exports, `exportfs -ra`. On Synology/TrueNAS, add the client in the share's NFS permissions.

### mount.nfs: Connection timed out
Means: The client couldn't reach the NFS server (down, firewall, or wrong IP).
Fix: `ping <server>`, `rpcinfo -p <server>`; open TCP 2049 (and 111 for NFSv3).

### mount.nfs: No such file or directory
Means: The export path doesn't exist on the server (or with NFSv4 the path is relative to the fsid=0 root).
Fix: `showmount -e <server>` lists exports; use that exact path.

### mount.nfs: Protocol not supported
Means: The client asked for an NFS version the server doesn't offer.
Fix: Try `-o vers=4.2`, `4.1` or `3`.

### mount.nfs: requested NFS version or transport protocol is not supported
Means: Same: version or TCP/UDP mismatch, or the NFS server service isn't running.
Fix: Check `systemctl status nfs-server` on the server; set `vers=` explicitly.

### mount: <dir>: bad option; for several filesystems (e.g. nfs, cifs) you might need a /sbin/mount.<type> helper program.
Means: The client lacks the mount helper.
Fix: Install `nfs-common` (Debian/Ubuntu) / `nfs-utils` (Fedora) or `cifs-utils`.

### nfs: server <server> not responding, still trying
Means: The NFS server stopped answering; processes using the mount hang (hard mounts wait forever).
Fix: Bring the server or network back. Use `soft,timeo=` only for non-critical data; add `_netdev,nofail` in fstab.

### nfs: server <server> OK
Means: The server answers again.
Fix: Nothing.

### Permission denied (NFS, root_squash)
Means: Root on the client is mapped to nobody on the server, so root can't write files it doesn't own.
Fix: Match uids between client and server, or use `no_root_squash` only for trusted hosts (or `all_squash,anonuid=` for app shares).

### NFS files show owner nobody / 4294967294
Means: NFSv4 id mapping failed: the domain in idmapd.conf differs, or the user doesn't exist on one side.
Fix: Set the same `Domain` in /etc/idmapd.conf on both, or use numeric ids (`nfs4_disable_idmapping`).

### exportfs: Failed to stat <path>: No such file or directory
Means: An /etc/exports entry points at a missing directory.
Fix: Create it or fix the path, then `exportfs -ra`.

### exportfs: <path> does not support NFS export
Means: The filesystem can't be exported (some FUSE mounts, overlay).
Fix: Export the underlying filesystem, or use `fsid=` for FUSE (with care).

### mount error(13): Permission denied
Means: The SMB server rejected the credentials, or the user may not access that share.
Fix: Check username, password and domain (`domain=WORKGROUP` or the NAS name); test with `smbclient //<server>/<share> -U <user>`.

### mount error(2): No such file or directory
Means: The share name doesn't exist on the server.
Fix: `smbclient -L //<server> -U <user>` lists shares.

### mount error(112): Host is down
Means: The client couldn't negotiate with the server, usually an SMB version mismatch (old NAS with SMB1 only, or a client forcing 1.0).
Fix: Set `vers=3.0` (or what the NAS supports); enable SMB2/3 on old NAS firmware instead of enabling SMB1.

### mount error(95): Operation not supported
Means: The requested SMB dialect or an option isn't supported by the server.
Fix: Try another `vers=`; remove unsupported options.

### mount error(115): Operation now in progress
Means: The connection to port 445 timed out (firewall or wrong IP).
Fix: `nc -vz <server> 445`.

### mount error(11): Resource temporarily unavailable
Means: The server or network was temporarily unable to complete the mount, often DNS or a busy NAS.
Fix: Retry; use the IP instead of a name.

### CIFS: VFS: cifs_mount failed w/return code = -13
Means: Kernel log for a permission-denied SMB mount.
Fix: Credentials or share permissions, as for error 13.

### CIFS: VFS: \\<server> has not responded in <n> seconds. Reconnecting...
Means: The SMB server stopped answering; the client will retry.
Fix: Check the server and network; mounts with `hard` hang until it returns.

### NT_STATUS_LOGON_FAILURE
Means: smbclient: wrong user name or password.
Fix: Check credentials; Samba users need `smbpasswd -a <user>` separately from the Linux password.

### NT_STATUS_ACCESS_DENIED
Means: Authenticated, but not allowed to that share or path.
Fix: Check `valid users`, share permissions and the filesystem permissions behind it.

### NT_STATUS_BAD_NETWORK_NAME
Means: No share by that name.
Fix: `smbclient -L //<server> -U <user>`.

### protocol negotiation failed: NT_STATUS_CONNECTION_RESET
Means: Client and server have no SMB dialect in common (often SMB1 disabled on one side).
Fix: Use SMB2/3 on both; set `client min protocol`/`server min protocol` consistently.

### smbd: Unable to connect to CUPS server localhost:631 - Connection refused
Means: A harmless Samba warning when printing is enabled without CUPS.
Fix: Add `load printers = no`, `printing = bsd`, `printcap name = /dev/null` to smb.conf.

### Error 0x80070035: The network path was not found
Means: Windows couldn't reach the share (name resolution, SMB disabled, firewall, or the server only offers SMB1).
Fix: Try `\\<ip>\<share>`; check port 445; make sure SMB2/3 is enabled on the server.

### Error 0x80004005: Unspecified error
Means: Windows' generic failure opening a share; often guest access blocked by policy or an SMB1-only server.
Fix: Use a real user account rather than guest; enable SMB2/3 on the server.

### You can't access this shared folder because your organization's security policies block unauthenticated guest access.
Means: Windows blocks guest-only shares.
Fix: Create a user on the NAS/Samba and connect with it.

### read: Connection reset by peer (sshfs)
Means: The SSH connection behind an sshfs mount dropped.
Fix: Use `-o reconnect,ServerAliveInterval=15`; remount with `fusermount -uz` if it's dead.

### fuse: mountpoint is not empty
Means: The FUSE mountpoint directory has files.
Fix: Empty it, or use `-o nonempty` (FUSE 2) knowing the files are hidden while mounted.

### fusermount: user has no write access to mountpoint <dir>
Means: You can't mount FUSE over a directory you don't own.
Fix: Use a directory you own, or mount as root with `allow_other`.

### fusermount: option allow_other only allowed if 'user_allow_other' is set in /etc/fuse.conf
Means: Letting other users (or containers) see a FUSE mount needs permission in fuse.conf.
Fix: Uncomment `user_allow_other` in /etc/fuse.conf.

### rclone: Failed to create file system for "<remote>:": didn't find section in config file
Means: The rclone remote name isn't in rclone.conf (or a different config file is used, for example under sudo).
Fix: `rclone listremotes`; run as the user who configured it or pass `--config`.

### rclone: couldn't connect: <details> token expired and there's no refresh token
Means: The OAuth token for a cloud remote expired.
Fix: `rclone config reconnect <remote>:`.

### rclone mount: Fatal error: failed to mount FUSE fs: fusermount: exec: "fusermount3": executable file not found in $PATH
Means: FUSE tools aren't installed.
Fix: Install `fuse3`.
