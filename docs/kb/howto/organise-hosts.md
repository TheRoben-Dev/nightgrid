# Organise hosts with groups, tags and icons

Keep a growing host list readable, and make groups do work for you: group power, fleet upgrades, Automation targets and UPS shutdowns all use them.

## What you end up with

- Groups that match how you act on machines (`media`, `infra`, `lab`, `family`)
- Tags for cross-cutting traits (`#docker`, `#proxmox`, `#arm`)
- Favourites at the top, colours and icons you recognise at a glance

## Part 1 · Groups

- [ ] In the host editor, set **Group** for each host. Pick groups by what you'd do to them together, not by location alone: everything in a group may be rebooted or upgraded together.
- [ ] Groups appear as targets in [Automation](view:automation) (click a group to select all of it), in **GROUP…** power actions, in Fleet → **Upgrade selected…** confirmation, and in Fleet → **UPS** shutdown plans.

## Part 2 · Tags, colours, icons, favourites

- [ ] Add tags; filter the sidebar with `#tag`.
- [ ] Give critical hosts a colour and an icon; star the ones you open most.
- [ ] Laptops, desktops and handhelds: tick **Client, not a server** so views never default to them.

## Part 3 · Per-view picks

- [ ] Each view remembers the host you last picked in it, so Docker can stay on your Docker host while System stays on the NAS.

## Part 4 · Dependencies

- [ ] **Settings → Alerts → Dependencies**: say which hosts depend on which (VMs on the hypervisor, apps on the NAS), so alerts fold under the real cause.

#### Check

- [ ] Filtering by a tag shows exactly the hosts you expect, and Automation offers your groups.

## Limits and gotchas

- A host has one group but any number of tags.
- Groups, tags and dependencies are in profile exports.

## Related

- [Run commands on many hosts with snippets and playbooks](kb:howto/automation)
- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
