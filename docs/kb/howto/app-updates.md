# Keep Nightgrid up to date

Nightgrid gets new code three ways: full app updates, Library content between releases, and interface fixes between releases. All three come from the project's GitHub Releases, are checked against the same signing key, and are controlled from one panel.

## What you end up with

- App updates offered after start-up, installed only when you click
- New and corrected Library articles without waiting for a release
- Small interface fixes for the exact version you run, applied on the next start, with a way back to the built-in interface

## Part 1 · App updates

- [ ] **Settings → GENERAL → UPDATES** shows the **Installed** version and **Check now**.
- [ ] **Check for updates on launch** (on to start) makes one request to GitHub Releases after start-up. When a newer version is out, **UPDATE AVAILABLE** offers **Install & restart**, **Later** or **Skip this version**. Nothing installs without your click.
- [ ] Terminals and tunnels close when Nightgrid restarts, so pick a quiet moment.

## Part 2 · Library content

- [ ] **Library content updates** (on to start) looks for newer articles after start-up and every 6 hours. The line under the switches says which content you're reading (built in, or a downloaded update with its article count) and when it last checked.
- [ ] **Check content** looks now. **Use built-in copy** deletes the download and goes back to the articles built into this version.
- [ ] The Library always works offline from its built-in copy.

## Part 3 · Interface fixes between releases

- [ ] **Interface fixes between releases** (on to start) looks for a fix built for exactly the version you run, after start-up and every 6 hours. It's a small download that can only change the interface: what the app is allowed to do stays as built.
- [ ] When one is ready, an **INTERFACE FIX** card lists what it fixes, with **Restart now** or **Next start**. It applies the next time Nightgrid starts.
- [ ] The line under the switches says **Interface built in**, or which fix is running and from when. **Check fixes** looks now; **Use built-in** deletes the fix so the next start uses the interface built into this version.

#### Check

- [ ] **Settings → GENERAL → UPDATES** shows **You're on the latest version.**

## Limits and gotchas

- If an interface fix ever fails to start, Nightgrid notices within a minute, goes back to the built-in interface and never uses that fix again.
- A fix for an older version is ignored once you update, because every release carries all fixes before it.
- Turning **Interface fixes between releases** off also stops a fix you already downloaded from being used.
- Fixes and Library content need GitHub Releases to be reachable; without it, the built-in copies are used.

## Related

- [Back up and move your Nightgrid setup](kb:howto/profiles)
- [Keep Nightgrid simple with feature presets](kb:howto/features)
