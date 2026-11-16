-- 0031 (M10x): iperf3 and ping results between two of your hosts, newest
-- 200 kept per pair. Throughput in bits per second, times in ms.
CREATE TABLE net_tests (
    id          INTEGER PRIMARY KEY,
    from_host   INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    to_host     INTEGER NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    target      TEXT NOT NULL,
    at          INTEGER NOT NULL,
    up_bps      REAL,
    down_bps    REAL,
    retransmits INTEGER,
    rtt_ms      REAL,
    jitter_ms   REAL,
    loss_pct    REAL,
    error       TEXT
) STRICT;
CREATE INDEX net_tests_pair_idx ON net_tests(from_host, to_host, at);
