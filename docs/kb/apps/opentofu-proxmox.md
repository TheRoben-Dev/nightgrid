# Proxmox VMs as code with OpenTofu

Describe VMs in a file, keep the file in git, and let OpenTofu (the open-source Terraform) create, change and destroy them on Proxmox. Rebuilding a lab becomes `tofu apply` instead of an afternoon of clicking.

![Describe, plan, apply.](fig:app-opentofu)

## What you end up with

- An API token for OpenTofu on your Proxmox node
- A small project that clones VMs from a cloud-init template
- `tofu plan` showing exactly what will change before anything does

## Before you start

- A cloud-init template on the node (see [Proxmox VM templates with cloud-init](kb:apps/cloud-init-templates)), for example VM 9000.
- OpenTofu installed on your workstation or a management VM (`tofu version`). The project runs from there, not on the node.

## Part 1 · An API token

- [ ] Nightgrid manages Proxmox over SSH, but OpenTofu's provider uses the Proxmox API. On the node:

```bash
pveum user add tofu@pve
pveum aclmod / -user tofu@pve -role PVEAdmin
pveum user token add tofu@pve iac --privsep 0
```

- [ ] Copy the token secret it prints (shown once). Keep it in an environment variable, never in the repo:

```bash
export PROXMOX_VE_API_TOKEN='tofu@pve!iac=<secret>'
```

## Part 2 · The project

- [ ] `main.tf` in a new folder (the `bpg/proxmox` provider is the actively maintained one):

```hcl
terraform {
  required_providers {
    proxmox = {
      source = "bpg/proxmox"
    }
  }
}

provider "proxmox" {
  endpoint = "https://pve.lan:8006/"
  insecure = true
}

resource "proxmox_virtual_environment_vm" "web" {
  name      = "web-01"
  node_name = "pve"

  clone {
    vm_id = 9000
  }

  cpu {
    cores = 2
  }

  memory {
    dedicated = 2048
  }

  initialization {
    ip_config {
      ipv4 {
        address = "dhcp"
      }
    }
    user_account {
      username = "admin"
      keys     = [trimspace(file("~/.ssh/id_ed25519.pub"))]
    }
  }
}
```

- [ ] `insecure = true` accepts the node's self-signed certificate. Pin the provider version in `required_providers` once it works.

## Part 3 · Plan and apply

```bash
tofu init
tofu plan
tofu apply
```

- [ ] `plan` lists what will be created, changed or destroyed. Read it every time; a renamed resource means destroy and recreate.
- [ ] Change `cores = 4`, `plan` again: it shows an in-place update. `tofu destroy` removes everything the project created.
- [ ] Commit `main.tf` and `.terraform.lock.hcl` to git. Keep `terraform.tfstate` out of git (it can contain secrets) and back it up instead.

#### Check

- [ ] `tofu apply` creates `web-01`, it boots with your key, and a second `tofu plan` says "No changes".

## In Nightgrid

- New VMs show up in the [Proxmox](view:proxmox) view right away; add each one as a host with **+** to manage what runs inside it.
- Changes made by hand in the Proxmox view or web UI show up as drift in the next `tofu plan`. Decide which side owns each VM, and keep hand edits for VMs OpenTofu doesn't manage.

## Limits and gotchas

- Some changes (disk shrink, moving to another storage) can force a destroy and recreate. `plan` says "must be replaced"; stop and think when you see it.
- The `PVEAdmin` role is broad. For a tighter token, create a role with only the VM, datastore and SDN privileges the provider docs list.
- Running OpenTofu from two machines against one state file corrupts it. Use one place, or a remote state backend with locking.

## Related

- [Proxmox VM templates with cloud-init](kb:apps/cloud-init-templates)
- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Ansible for your homelab](kb:apps/ansible)
