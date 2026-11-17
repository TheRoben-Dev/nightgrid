# Semaphore UI for Ansible

A web interface for running Ansible playbooks (and Terraform/OpenTofu, Bash or PowerShell scripts): store inventories and SSH keys once, run playbooks with a button or on a schedule, and keep a log of every run and who started it.

![Semaphore runs playbooks like a control host.](fig:app-ansible)

## What you end up with

- Semaphore with its built-in database, behind your proxy or on the LAN
- A project with an inventory, an SSH key and a git repository of playbooks
- A scheduled "update all hosts" task with run history

## Before you start

- Your playbooks in a git repository (Forgejo, Gitea or GitHub). See [Ansible for your homelab](kb:apps/ansible) for a first playbook.
- An SSH key pair that can log in to your hosts as a sudo user. Make a dedicated one with **NEW KEY…** in **Settings → Connections → SSH keys** and add it to each host with **+** in the **WHERE THEY WORK** table.

## Part 1 · The stack

- [ ] Create an encryption key for stored secrets: `head -c32 /dev/urandom | base64`.
- [ ] [Docker](view:docker) → **+ New stack** named `semaphore`, with `ADMIN_PASSWORD` and `ENC_KEY` in `.env`:

```yaml
services:
  semaphore:
    image: semaphoreui/semaphore:latest
    restart: unless-stopped
    environment:
      SEMAPHORE_DB_DIALECT: bolt
      SEMAPHORE_ADMIN: admin
      SEMAPHORE_ADMIN_NAME: Admin
      SEMAPHORE_ADMIN_EMAIL: admin@localhost
      SEMAPHORE_ADMIN_PASSWORD: ${ADMIN_PASSWORD}
      SEMAPHORE_ACCESS_KEY_ENCRYPTION: ${ENC_KEY}
      TZ: Etc/UTC
    volumes:
      - ./data:/var/lib/semaphore
      - ./config:/etc/semaphore
    ports:
      - "3000:3000"
```

- [ ] Open port 3000 and log in as `admin`. Pin a version tag once it works.
- [ ] `bolt` is an embedded database, fine for one admin. Use PostgreSQL (`SEMAPHORE_DB_DIALECT: postgres` plus `SEMAPHORE_DB_HOST`, `_USER`, `_PASS`, `_DB`) for several users.

## Part 2 · A project

- [ ] **New project** → `homelab`.
- [ ] **Key Store** → add the SSH private key (type **SSH Key**), and a "None" key for public git repos or a deploy key for private ones.
- [ ] **Repositories** → your playbook repo URL and branch.
- [ ] **Inventory** → type **Static**, paste an INI inventory:

```ini
[servers]
web-01 ansible_host=192.0.2.21
nas-01 ansible_host=192.0.2.22

[servers:vars]
ansible_user=admin
```

- [ ] **Task Templates** → New: playbook file (`update.yml`), the inventory, the repo, and **Vault** password if your playbooks use one.

## Part 3 · Run and schedule

- [ ] **Run** the template. The live log shows each task per host; a failed host is red with the error.
- [ ] Edit the template → **Schedule** with a cron expression such as `0 4 * * 0` for Sundays at 04:00.

#### Check

- [ ] The task finishes **Success** for every host, and **History** lists it with the time and user.

## In Nightgrid

- [Fleet](view:fleet) → **Updates** shows pending updates for all hosts and can apply them without Ansible. Use Semaphore when updates are only one step of a longer playbook.
- Add Semaphore to [Services](view:services); a scheduled run that stops working shows up as a failed task inside Semaphore, so check its history weekly or add notifications in its settings.
- Every change a playbook makes to `/etc` shows in [System](view:system) → **config** → **Compare** against a saved baseline.

## Limits and gotchas

- The container has its own Ansible. Collections your playbooks need go in a `requirements.yml` in the repo, which Semaphore installs before the run.
- Keys in the Key Store are encrypted with `SEMAPHORE_ACCESS_KEY_ENCRYPTION`; lose that value and you re-enter every secret.
- Hosts must have Python 3 for most Ansible modules.

## Related

- [Ansible for your homelab](kb:apps/ansible)
- [Forgejo git server](kb:apps/forgejo)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
