# Errors: git

Messages from git when cloning, pulling and pushing, including the stacks-from-git feature that runs git on your hosts.

### fatal: not a git repository (or any of the parent directories): .git
Means: The current directory isn't inside a git repository.
Fix: `cd` into the repository, or `git init`/`git clone` first.

### fatal: destination path '<dir>' already exists and is not an empty directory.
Means: git clone won't write into a non-empty directory.
Fix: Clone elsewhere, or remove/rename the existing folder after checking it.

### fatal: repository '<url>' not found
Means: The URL is wrong, or it's a private repository and your credentials can't see it (GitHub says "not found" rather than "forbidden").
Fix: Check the URL; use a token or deploy key with read access.

### fatal: Authentication failed for '<url>'
Means: The HTTPS username/password or token was rejected. GitHub doesn't accept account passwords for git.
Fix: Use a personal access token (or a fine-grained token with Contents: read) as the password, or switch to SSH.

### remote: Support for password authentication was removed on August 13, 2021.
Means: GitHub requires a token or SSH key, not a password.
Fix: Create a token and use it as the password, or use an SSH remote.

### git@github.com: Permission denied (publickey).
Means: GitHub didn't accept any SSH key you offered.
Fix: `ssh -T git@github.com` to test. Add your public key (or a deploy key) to GitHub; point `IdentityFile` at it in ~/.ssh/config.

### ERROR: Repository not found.
Means: Over SSH, the key works but has no access to that repo (a deploy key for another repo, or a typo).
Fix: Check the path and which account or deploy key the key belongs to.

### fatal: Could not read from remote repository.
Means: The SSH connection to the git host failed or was refused (key or access).
Fix: The line above it gives the reason; `GIT_SSH_COMMAND="ssh -v" git fetch` for detail.

### ERROR: Permission to <owner>/<repo>.git denied to <user>.
Means: The authenticated account can read but not push (or not even read).
Fix: Get write access, or push to your own fork.

### ! [rejected] <branch> -> <branch> (fetch first)
Means: The remote has commits you don't have; git won't overwrite them.
Fix: `git pull --rebase` (or merge), resolve conflicts, then push.

### ! [rejected] <branch> -> <branch> (non-fast-forward)
Means: Your branch is behind or has diverged from the remote.
Fix: Integrate the remote changes first. Force-pushing discards others' work; avoid it on shared branches.

### ! [remote rejected] <branch> -> <branch> (protected branch hook declined)
Means: The branch is protected (reviews, status checks) and refuses direct pushes.
Fix: Push to another branch and open a pull request.

### error: failed to push some refs to '<url>'
Means: Summary line; the reason is in the lines above.
Fix: Read the "rejected" line above it.

### error: Your local changes to the following files would be overwritten by merge:
Means: You have uncommitted edits to files the pull would change.
Fix: Commit them, `git stash` them, or discard them (`git restore <file>`) before pulling.

### error: Your local changes to the following files would be overwritten by checkout:
Means: Switching branches would clobber uncommitted changes.
Fix: Commit or stash first.

### CONFLICT (content): Merge conflict in <file>
Means: Both sides changed the same lines; git needs you to choose.
Fix: Edit the file (between `<<<<<<<` and `>>>>>>>`), `git add <file>`, then `git commit` or `git rebase --continue`.

### Automatic merge failed; fix conflicts and then commit the result.
Means: One or more files conflict.
Fix: `git status` lists them; resolve, add, commit. `git merge --abort` to back out.

### fatal: refusing to merge unrelated histories
Means: The two branches share no common commit (often a repo initialized separately on both sides).
Fix: If intended, `git pull --allow-unrelated-histories`; otherwise check you're pulling the right repo.

### fatal: Need to specify how to reconcile divergent branches.
Means: Newer git asks whether to merge or rebase when pulling diverged branches.
Fix: `git pull --rebase` or `--no-rebase`, or set `git config pull.rebase false` (or true) once.

### hint: You have divergent branches and need to specify how to reconcile them.
Means: Same as above.
Fix: Choose merge or rebase as above.

### fatal: detected dubious ownership in repository at '<dir>'
Means: The repository belongs to another user than the one running git (a safety check since git 2.35.2), common for repos under /opt or /srv, or run with sudo.
Fix: Run git as the owning user, or trust it: `git config --global --add safe.directory <dir>`.

### fatal: unable to access '<url>': Could not resolve host: <host>
Means: DNS or network failure reaching the git host.
Fix: Check the host's network and DNS.

### fatal: unable to access '<url>': SSL certificate problem: unable to get local issuer certificate
Means: git can't verify the server's TLS chain (private Gitea/Forgejo with a self-signed certificate, or an intercepting proxy).
Fix: Add the CA to the system store, or for that remote only: `git config http.<url>.sslCAInfo /path/ca.pem`.

### fatal: unable to access '<url>': The requested URL returned error: 403
Means: Authenticated but not allowed (token without the right scope, or IP restrictions).
Fix: Check the token's permissions for that repo.

### error: RPC failed; curl 92 HTTP/2 stream 0 was not closed cleanly: CANCEL (err 8)
Means: A large HTTP transfer was cut off (proxies, unstable connections, HTTP/2 issues).
Fix: Retry; `git config --global http.version HTTP/1.1`; clone shallow with `--depth 1` for huge repos.

### fatal: early EOF
Means: The connection dropped during clone or fetch.
Fix: Retry, try a shallow clone, or SSH instead of HTTPS.

### error: pathspec '<name>' did not match any file(s) known to git
Means: The branch or file name doesn't exist (locally), often a remote branch not fetched yet.
Fix: `git fetch` then `git switch <branch>`; check spelling.

### fatal: ambiguous argument '<ref>': unknown revision or path not in the working tree.
Means: git can't find that commit, branch or path.
Fix: `git fetch`; check `git log --oneline` for the right ref.

### You are in 'detached HEAD' state.
Means: You checked out a commit or tag instead of a branch. Not an error.
Fix: Create a branch if you'll commit: `git switch -c <name>`.

### fatal: The current branch <branch> has no upstream branch.
Means: The branch doesn't track a remote branch yet.
Fix: `git push -u origin <branch>` once.

### error: src refspec <branch> does not match any
Means: You pushed a branch that doesn't exist locally (often `master` vs `main`, or no commits yet).
Fix: `git branch` to see the real name; commit something first in a new repo.

### fatal: index file corrupt
Means: .git/index is damaged (crash, disk issue).
Fix: `rm .git/index && git reset` rebuilds it from HEAD (uncommitted staging is lost, files stay).

### fatal: Unable to create '<dir>/.git/index.lock': File exists.
Means: Another git process is running, or one crashed and left the lock.
Fix: If no git is running, delete `.git/index.lock`.

### warning: LF will be replaced by CRLF the next time Git touches it
Means: Line-ending conversion is on (core.autocrlf), usually on Windows.
Fix: For server files and scripts, use `core.autocrlf=input` so files keep LF.

### fatal: could not read Username for '<url>': terminal prompts disabled
Means: git needed HTTPS credentials in a non-interactive run (cron, a service, Nightgrid's git deploy).
Fix: Configure a credential helper with a token, or use an SSH deploy key on the host.

### Host key verification failed. fatal: Could not read from remote repository.
Means: The git host's SSH key isn't in known_hosts for the user running git (common for root or service users).
Fix: As that user: `ssh -T git@<host>` once to accept it, after checking the fingerprint against the host's published ones.

### error: object file <path> is empty
Means: A git object was truncated (crash or full disk).
Fix: Remove empty objects (`find .git/objects -size 0 -delete`), `git fetch`, then `git fsck`. Reclone if needed.

### fatal: shallow update not allowed
Means: Pushing from a shallow clone to a remote that lacks the history.
Fix: `git fetch --unshallow`, then push.

### error: unable to unlink old '<file>': Permission denied
Means: git can't replace a file during checkout/pull because of permissions (files owned by root or a container user).
Fix: Fix ownership of the working tree (`chown -R`), or run git as the owner.
