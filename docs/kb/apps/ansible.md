# Ansible for your homelab

Describe how each server should be configured (packages, users, files, services) and apply it repeatably with Ansible over the same SSH access Nightgrid uses.

![A control host running playbooks over SSH.](fig:app-ansible)

## What you end up with

- An inventory of your hosts in groups
- Playbooks for baseline setup that you can rerun safely
- Runs launched from a control host, from Nightgrid's terminal or Automation

## Before you start

- A control machine: your computer, or a small always-on server. Ansible needs nothing on the managed hosts but SSH and Python.
- Your key on every host (see [Set up and rotate SSH keys](kb:howto/ssh-keys)).

## Part 1 · Install and inventory

- [ ] On the control host: `sudo apt install -y ansible` (or `pipx install ansible`).
- [ ] `~/homelab/inventory.ini`:

```ini
[docker]
media ansible_host=192.168.1.10
apps  ansible_host=192.168.1.11

[dns]
dns1 ansible_host=192.168.1.2

[all:vars]
ansible_user=admin
```

- [ ] Test: `ansible all -i inventory.ini -m ping`.

## Part 2 · A baseline playbook

- [ ] `~/homelab/base.yml`:

```yaml
- hosts: all
  become: true
  tasks:
    - name: Base packages
      ansible.builtin.apt:
        name: [htop, curl, unattended-upgrades, fail2ban, smartmontools]
        state: present
        update_cache: true
    - name: Key-only SSH
      ansible.builtin.copy:
        dest: /etc/ssh/sshd_config.d/10-hardening.conf
        content: |
          PasswordAuthentication no
          PermitRootLogin no
        mode: "0644"
      notify: restart ssh
  handlers:
    - name: restart ssh
      ansible.builtin.service:
        name: ssh
        state: restarted
```

- [ ] Dry run, then apply:

```sh
ansible-playbook -i inventory.ini base.yml --check --diff
ansible-playbook -i inventory.ini base.yml -K
```

## Part 3 · With Nightgrid

- [ ] Save the run as a snippet in [Automation](view:automation) targeting the control host: `cd ~/homelab && ansible-playbook -i inventory.ini {{playbook=base.yml}} --diff`.
- [ ] After a run, [System](view:system) → **config** → **Compare** shows exactly what changed on each host; save a new baseline once you're happy.
- [ ] Keep the playbooks in git (see [Forgejo git server](kb:apps/forgejo)).

#### Check

- [ ] Running the playbook twice reports `changed=0` the second time.

## Limits and gotchas

- `-K` asks for the sudo password; for unattended runs give the Ansible user passwordless sudo, carefully.
- Ansible secrets belong in `ansible-vault`, never in plain inventory files.

## Related

- [Run commands on many hosts with snippets and playbooks](kb:howto/automation)
- [Track config drift and host history](kb:howto/config-drift)
- [Hardening sshd](kb:basics/sshd-hardening)
