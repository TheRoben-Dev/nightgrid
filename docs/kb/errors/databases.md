# Errors: databases

Messages from PostgreSQL, MariaDB/MySQL, Redis, SQLite and MongoDB, the databases most self-hosted apps run on.

### FATAL: password authentication failed for user "<user>"
Means: PostgreSQL rejected the password, or the user doesn't exist.
Fix: Check the app's DB password matches. Note that POSTGRES_PASSWORD in a container only applies when the data directory is first created; changing it later does nothing.

### FATAL: role "<user>" does not exist
Means: That PostgreSQL user was never created (common with `psql` as your Linux user).
Fix: `sudo -u postgres psql`, or create the role. Use the user the app was configured with.

### FATAL: database "<name>" does not exist
Means: The database hasn't been created (or the name differs in case).
Fix: `createdb <name>` or set POSTGRES_DB on a fresh volume.

### FATAL: no pg_hba.conf entry for host "<ip>", user "<user>", database "<name>", SSL off
Means: pg_hba.conf has no rule allowing that client address and method.
Fix: Add a line for the client's subnet (for example `host all all 172.16.0.0/12 scram-sha-256`), then reload PostgreSQL.

### FATAL: Peer authentication failed for user "<user>"
Means: Local socket connections use peer auth, which requires the Linux user name to match the DB user.
Fix: Connect as that Linux user (`sudo -u <user> psql`), use `-h 127.0.0.1` for password auth, or change pg_hba.conf.

### FATAL: sorry, too many clients already
Means: All `max_connections` slots are used: a connection leak or many apps on one server.
Fix: `SELECT count(*), application_name FROM pg_stat_activity GROUP BY 2;` to find who. Fix the leak, add a pooler (PgBouncer), or raise max_connections moderately.

### FATAL: the database system is starting up
Means: PostgreSQL is still recovering or starting; connections are refused for now.
Fix: Wait; an app depending on it should retry (or use a healthcheck with depends_on).

### FATAL: the database system is in recovery mode
Means: PostgreSQL crashed and is replaying WAL.
Fix: Wait, and look in the log for why it crashed (often OOM or a full disk).

### FATAL: database files are incompatible with server
Means: The data directory was created by a different major PostgreSQL version than the one now running (an image tag bumped from 15 to 16, for example).
Fix: Pin the old major version again, then upgrade properly: dump and restore, or pg_upgrade. Never change Postgres major tags on an existing volume.

### DETAIL: The data directory was initialized by PostgreSQL version <n>, which is not compatible with this version <n>.
Means: Same: major version mismatch between data and server.
Fix: Go back to the matching image tag; migrate with `pg_dumpall` into a new volume.

### could not connect to server: Connection refused Is the server running on host "<host>" and accepting TCP/IP connections on port 5432?
Means: Nothing listens on that host and port: PostgreSQL is down or listens only on localhost (`listen_addresses`).
Fix: Check the service; set `listen_addresses = '*'` (and pg_hba) for remote access, or connect via the container network.

### connection to server on socket "/var/run/postgresql/.s.PGSQL.5432" failed: No such file or directory
Means: The local PostgreSQL socket doesn't exist: the server isn't running, or the socket is elsewhere.
Fix: `systemctl status postgresql`; use `-h localhost` to use TCP.

### PANIC: could not write to file "<file>": No space left on device
Means: PostgreSQL's disk filled up; it stops to protect data.
Fix: Free space (old WAL from a stuck replication slot is common: `SELECT * FROM pg_replication_slots;`), then restart.

### ERROR: duplicate key value violates unique constraint "<name>"
Means: An insert reused a key that must be unique, often a sequence that fell behind after a restore.
Fix: For sequences: `SELECT setval('<seq>', (SELECT max(id) FROM <table>));`.

### ERROR: relation "<name>" does not exist
Means: The table doesn't exist in that database or schema: migrations haven't run, or you're connected to the wrong DB.
Fix: Run the app's migrations; check `\dt` in the right database.

### ERROR: permission denied for schema public
Means: Since PostgreSQL 15, ordinary users can't create objects in the public schema by default.
Fix: As the owner: `GRANT ALL ON SCHEMA public TO <user>;` or make the user the database owner.

### ERROR: could not open extension control file "<path>": No such file or directory
Means: The extension (pgvector, postgis, vectors) isn't installed in this PostgreSQL image.
Fix: Use the image the app documents (Immich needs its own Postgres image with the vector extension).

### WARNING: database "<name>" has a collation version mismatch
Means: The OS's glibc collation changed (image upgrade), so indexes on text may be inconsistent.
Fix: `REINDEX DATABASE <name>;` then `ALTER DATABASE <name> REFRESH COLLATION VERSION;`.

### pg_dump: error: server version: <n>; pg_dump version: <n>
Means: pg_dump is older than the server; it refuses to dump.
Fix: Use the pg_dump from the server's version (run it inside the database container).

### pg_restore: error: input file appears to be a text format dump. Please use psql.
Means: The backup is plain SQL, not the custom format.
Fix: `psql -f <dump.sql>` instead.

### ERROR 1045 (28000): Access denied for user '<user>'@'<host>' (using password: YES)
Means: MariaDB/MySQL rejected the user, password or the host it connects from.
Fix: Users are per host: `CREATE USER '<user>'@'%'` for remote clients. Check the password; MYSQL_ROOT_PASSWORD only applies on first init.

### ERROR 1045 (28000): Access denied for user 'root'@'localhost' (using password: NO)
Means: root on that server needs a password (or uses unix_socket auth as the system root).
Fix: `sudo mysql` (socket auth) or `mysql -u root -p`.

### ERROR 2002 (HY000): Can't connect to local MySQL server through socket '/var/run/mysqld/mysqld.sock' (2)
Means: The server isn't running, or the client looks for the socket in the wrong place.
Fix: `systemctl status mariadb`; use `-h 127.0.0.1` for TCP.

### ERROR 2003 (HY000): Can't connect to MySQL server on '<host>' (111)
Means: Nothing listens on that host's 3306 (stopped, or bind-address = 127.0.0.1).
Fix: Check the service and `bind-address`; firewall.

### ERROR 1049 (42000): Unknown database '<name>'
Means: The database doesn't exist.
Fix: `CREATE DATABASE <name>;` or set MYSQL_DATABASE on a fresh volume.

### ERROR 1130 (HY000): Host '<ip>' is not allowed to connect to this MariaDB server
Means: No user account matches that client host.
Fix: Create the user for that host or `%`, and grant privileges.

### ERROR 1040 (HY000): Too many connections
Means: max_connections reached.
Fix: Find the leaking client (`SHOW PROCESSLIST;`); raise max_connections if the load is real.

### ERROR 1205 (HY000): Lock wait timeout exceeded; try restarting transaction
Means: A query waited too long for a row lock held by another transaction.
Fix: Find long transactions (`SHOW ENGINE INNODB STATUS`); usually an app bug or a stuck job.

### ERROR 1213 (40001): Deadlock found when trying to get lock; try restarting transaction
Means: Two transactions waited on each other; one was rolled back.
Fix: The app should retry. Frequent deadlocks need query or index changes.

### [ERROR] InnoDB: Unable to lock ./ibdata1 error: 11
Means: Another mysqld (or a second container) is using the same data directory.
Fix: Make sure only one server uses that volume.

### [ERROR] InnoDB: Upgrade after a crash is not supported.
Means: A newer MariaDB/MySQL won't start on data from a crashed older one.
Fix: Start the old version once so it recovers cleanly, shut it down properly, then upgrade.

### [Warning] Aborted connection <n> to db: '<name>' user: '<user>' host: '<ip>' (Got an error reading communication packets)
Means: A client disconnected without closing properly (timeouts, crashes, network).
Fix: Usually harmless in small numbers; frequent ones point to wait_timeout mismatches with the app's pool.

### Table '<name>' doesn't exist
Means: The app's tables aren't there: migrations not run, or wrong database selected.
Fix: Run the app's setup/migrations.

### Incorrect string value: '\xF0\x9F<details>' for column <name>
Means: An emoji (4-byte UTF-8) didn't fit a utf8 (utf8mb3) column.
Fix: Convert the database and tables to utf8mb4.

### mysqldump: Got error: 1044: Access denied for user '<user>'@'<host>' to database '<name>' when using LOCK TABLES
Means: The dump user lacks LOCK TABLES privilege.
Fix: Grant it, or use `--single-transaction` for InnoDB.

### Could not connect to Redis at <host>:<port>: Connection refused
Means: Redis isn't running or listens elsewhere (protected mode binds to localhost).
Fix: Check the container; apps in other containers must use the Redis container's name, not localhost.

### NOAUTH Authentication required.
Means: Redis has a password (requirepass) and the client didn't send it.
Fix: Set the password in the app's Redis URL: `redis://:password@redis:6379`.

### WRONGPASS invalid username-password pair or user is disabled.
Means: The Redis password or ACL user is wrong.
Fix: Match the app's setting to Redis's requirepass/ACL.

### MISCONF Redis is configured to save RDB snapshots, but it's currently unable to persist to disk.
Means: Redis can't write its dump file (full disk or permissions), so it refuses writes.
Fix: Free space or fix the data directory's permissions; check `redis-cli config get dir`.

### OOM command not allowed when used memory > 'maxmemory'.
Means: Redis reached its memory limit with a no-eviction policy.
Fix: Raise maxmemory, or set an eviction policy (`allkeys-lru`) if it's a cache.

### WARNING Memory overcommit must be enabled! Without it, a background save or replication may fail under low memory condition.
Means: Redis recommends `vm.overcommit_memory = 1` on the host.
Fix: `sudo sysctl vm.overcommit_memory=1` and persist it in /etc/sysctl.d/.

### LOADING Redis is loading the dataset in memory
Means: Redis is starting and reading its dump; commands are refused until it's done.
Fix: Wait; large datasets take a while.

### database is locked
Means: SQLite: another process holds a write lock (or the database is on a network filesystem with broken locking).
Fix: Don't put SQLite databases on NFS/SMB shares; stop duplicate instances of the app. Retry for short locks.

### sqlite3.OperationalError: database is locked
Means: Python's wording of the SQLite lock.
Fix: As above; enable WAL mode and keep the DB on local disk.

### database disk image is malformed
Means: The SQLite file is corrupt: a crash during write, copying it while in use, or network storage.
Fix: Restore from backup. Try `sqlite3 <db> ".recover" | sqlite3 fixed.db`. Back up SQLite with `.backup`, not by copying the live file.

### unable to open database file
Means: SQLite can't open or create the file: the directory doesn't exist or isn't writable (SQLite needs to write journal files beside it).
Fix: Check the directory's permissions, not only the file's. In containers, check volume ownership (PUID/PGID).

### attempt to write a readonly database
Means: The database file or its directory isn't writable by the app's user.
Fix: chown the data directory to the app's uid.

### MongoServerError: Authentication failed.
Means: Wrong MongoDB user, password or authSource.
Fix: Add `?authSource=admin` to the connection string when the user was created in admin.

### MongoNetworkError: connect ECONNREFUSED <ip>:27017
Means: MongoDB isn't listening at that address.
Fix: Check the container and bindIp.

### WARNING: MongoDB 5.0+ requires a CPU with AVX support, and your current system does not appear to have that!
Means: The CPU (or a VM's CPU model) lacks AVX, so MongoDB 5+ crashes with "Illegal instruction".
Fix: Set the VM CPU type to `host`, or use MongoDB 4.4 (UniFi and Omada often hit this).

### Illegal instruction (core dumped) mongod
Means: Same AVX requirement as above.
Fix: CPU type `host` in Proxmox, or an older MongoDB.
