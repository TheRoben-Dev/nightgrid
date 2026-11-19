-- 0051 (1.8.2): clean up uptime history recorded before downs had to be
-- confirmed (D-181). Two kinds of false downtime are removed:
--
-- 1. Client devices (role 'client', D-133) going away isn't downtime: their
--    down seconds become asleep and their outages are dropped.
-- 2. Outages shorter than 150 s (one failed check or poll, the minute after
--    the computer woke) are dropped and their seconds become unknown time
--    (taken off `down`, added nowhere), as the recorder now does.

UPDATE uptime_hours
SET asleep = asleep + down, down = 0
WHERE kind = 'host' AND down > 0 AND subject IN (SELECT id FROM hosts WHERE role = 'client');

DELETE FROM outages
WHERE kind = 'host' AND subject IN (SELECT id FROM hosts WHERE role = 'client');

UPDATE uptime_hours
SET down = MAX(0, down - (
    SELECT COALESCE(SUM(MIN(o.end, uptime_hours.hour + 3600) - MAX(o.start, uptime_hours.hour)), 0)
    FROM outages o
    WHERE o.kind = uptime_hours.kind AND o.subject = uptime_hours.subject
      AND o.end - o.start < 150
      AND o.start < uptime_hours.hour + 3600 AND o.end > uptime_hours.hour
))
WHERE down > 0;

DELETE FROM outages WHERE end - start < 150;

-- Hours left with nothing in them carry no information.
DELETE FROM uptime_hours WHERE up = 0 AND down = 0 AND asleep = 0;
