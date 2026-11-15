-- 0017 (M10k): Wake-on-LAN and planned sleep. The MAC address the magic
-- packet wakes, an optional relay host on the sleeping host's LAN that sends
-- it, and the local hours the host is expected to be asleep ("23:00-07:00"),
-- during which "doesn't answer" means asleep, not offline.

ALTER TABLE hosts ADD COLUMN mac_address TEXT;
ALTER TABLE hosts ADD COLUMN wake_relay_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL;
ALTER TABLE hosts ADD COLUMN sleep_window TEXT;
