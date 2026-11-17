# Keep server clocks in sync

Wrong clocks break TLS, TOTP codes, log ordering, backups and cluster quorum. Make sure every server syncs time and uses the right time zone.

![One chrony server keeps the whole LAN on time.](fig:lx-time-sync)

## Part 1 · Check

```sh
timedatectl
```

- [ ] "System clock synchronized: yes" and "NTP service: active" is what you want. {check: time-sync}

## Part 2 · Fix it

- [ ] Most distros run `systemd-timesyncd` or `chrony`. If neither is active:

```sh
sudo apt install -y chrony
sudo systemctl enable --now chrony
chronyc tracking
```

- [ ] Set the zone: `sudo timedatectl set-timezone Europe/Berlin` (yours from `timedatectl list-timezones`). Many prefer UTC on servers; pick one rule for all.
- [ ] Use your router or a local NTP server if the network blocks outbound NTP: `server 192.168.1.1 iburst` in `/etc/chrony/chrony.conf`.

## Part 3 · In Nightgrid

- [ ] Nightgrid's POWER SCHEDULE and timers use the host's clock; a wrong zone means jobs run at the wrong hour.
- [ ] An Automation snippet across all hosts: `timedatectl show -p NTPSynchronized -p Timezone`.

#### Check

- [ ] Every host reports synchronized and the expected zone. {check: time-sync}

## Limits and gotchas

- VMs can drift after suspend or host load; chrony corrects faster than timesyncd.
- Proxmox clusters need time within a second or two between nodes.

## Related

- [Schedule jobs with cron and timers](kb:howto/schedules)
- [Run commands on many hosts with snippets and playbooks](kb:howto/automation)
