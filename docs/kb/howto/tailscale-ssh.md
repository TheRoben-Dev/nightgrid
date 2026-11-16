# Use Tailscale SSH hosts

Log in to machines where Tailscale handles SSH authentication (`tailscale up --ssh`), including check-mode policies that ask you to re-approve in a browser.

## What you end up with

- Hosts that need no SSH key on the server: the tailnet's ACL decides who may log in
- A clear **Approve in browser** card when a policy needs re-authentication
- Monitoring and background features that keep working once approved

## Before you start

- Tailscale on the server with SSH turned on.
- Either the Tailscale app on this computer or Nightgrid's **Built-in Tailscale** (see [Reach your servers away from home](kb:howto/remote-access)).
- An `ssh` rule in your tailnet policy that lets your user log in as the target user.

## Part 1 · Turn on Tailscale SSH on the server

- [ ] In a [Terminal](view:terminal) on the server:

```sh
sudo tailscale set --ssh
```

- [ ] A minimal policy rule (in the tailnet's access controls) for your own devices:

```json
"ssh": [
  { "action": "accept", "src": ["autogroup:member"], "dst": ["autogroup:self"], "users": ["autogroup:nonroot", "root"] }
]
```

Use `"action": "check"` instead to require a fresh browser approval every so often.

## Part 2 · Add the host

- [ ] **+** in the sidebar, address = the MagicDNS name or 100.x address, login **SSH agent**. No key file is needed.
- [ ] The host editor notes when an address is a tailnet one.
- [ ] Connect. With a check-mode rule, a **Tailscale SSH** card shows **Approve in browser** and the terminal says *Waiting for approval*. Nightgrid waits up to 5 minutes.

#### Check

- [ ] The host connects without a password and shows on the [Overview](view:overview).

## Limits and gotchas

- If the policy says no, the error says so and points at the policy's `ssh` rules. Fix it in the admin console, not in Nightgrid.
- Check mode re-asks after its period (12 hours by default). Background monitoring pauses until you approve again.
- Tailscale SSH only answers on the tailnet address. Saving the LAN address uses normal OpenSSH instead.

## Related

- [Reach your servers away from home](kb:howto/remote-access)
- [Tailscale subnet router](kb:apps/tailscale-subnet-router)
- [When a host won't connect](kb:howto/troubleshoot-connection)
