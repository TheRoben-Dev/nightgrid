# Deploy stacks from git

Keep compose files in a git repository and deploy a reviewed commit to a host, with a snapshot before each deploy and one-click rollback.

## What you end up with

- Each stack's compose file versioned in git, with history and review
- A **Check** that shows new commits for that stack, and a diff before every deploy
- Rollback to the previous deploy without touching git by hand

## Before you start

- A git repository (self-hosted Forgejo, see [Forgejo git server](kb:apps/forgejo), or any hosted one).
- The host's own access to it: add the host's public key as a read-only deploy key, or set up a credential helper on the host. Nightgrid never sends your credentials to the host.
- `git` installed on the host.

## Part 1 · Lay out the repository

- [ ] One folder per stack works well:

```
stacks/
  media/compose.yaml
  monitoring/compose.yaml
  dns/compose.yaml
```

- [ ] Keep secrets out of git. Use the stack's **Vault** and **Deploy .env…** for them; add `.env` to `.gitignore`.

## Part 2 · Connect a stack

- [ ] [Docker](view:docker) → **From git…** (above the list).
- [ ] Repository address, branch, a folder on the host for the checkout, and the stack's folder inside the repo.
- [ ] **Clone** clones it on the host.

## Part 3 · Deploy

- [ ] Push a change to the repository.
- [ ] **Check** on the stack shows new commits.
- [ ] **Review & deploy…** shows the diff. **Deploy and up** saves a snapshot, moves the checkout to that commit and brings the stack up.

#### Check

- [ ] The containers restarted with the change, and System → **timeline** on the host shows the deploy.

## Part 4 · Roll back

- [ ] **Roll back to …** returns to the previous deploy, compose files and images.
- [ ] Then fix forward in git, so the repository and the host agree again.

## Limits and gotchas

- Files edited directly on the host block a deploy until you commit or undo them there. That's on purpose: nothing gets silently overwritten.
- Nightgrid deploys when you press the button. For automatic deploys on push, use a CI runner (see [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)).
- Submodules and git LFS aren't handled specially; they work if the host's git is set up for them.

## Related

- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Forgejo git server](kb:apps/forgejo)
- [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)
- [Update containers without fear](kb:howto/container-updates)
