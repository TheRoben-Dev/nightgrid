# Shortcuts, palette and themes

Move around Nightgrid without the mouse, and make it look and sound the way you like: themes, a custom palette, scale, effects and UI sounds.

## Part 1 · Keyboard

- [ ] `Mod` is Cmd on macOS, Ctrl elsewhere. Views: Overview `Mod+1` … Network `Mod+0`, Fleet `Mod+Shift+U`, Proxmox `Mod+Shift+P`, Library `Mod+Shift+L`, Settings `Mod+,`.
- [ ] `Mod+K` opens the command palette: hosts, views, snippets, playbooks, actions and Library articles. Inside a terminal it's `Ctrl+Shift+K`.
- [ ] `Mod+J` opens DRIFT; `Mod+B` collapses the sidebar.
- [ ] **Back and forward**: the mouse side buttons, `Cmd+[` / `Cmd+]` on macOS, `Alt+Left` / `Alt+Right` on Windows and Linux, the **‹ ›** buttons at the top left, or a two-finger sideways swipe on a trackpad. Every view, tab, host pick and Library article is a step, so an accidental click is one button away from undone. The keys are left alone while you type in a field or editor, and in a terminal on Windows and Linux (shells use `Alt+Left` / `Alt+Right` to move by word); the mouse buttons work everywhere. **Settings → APPEARANCE → DISPLAY → Swipe to go back** turns the swipe off.
- [ ] **Settings → Terminal → Keybindings** lists everything.

## Part 2 · Themes

- [ ] **Settings → Appearance**: NIGHTGRID, ICE, BLOODLINE or CUSTOM.
- [ ] CUSTOM: pick each palette slot (surfaces, text, accent, info, active, danger, OK, warning, neutral highlight) from curated swatches that are contrast-checked. **Start from** copies a built-in scheme; **Reset** goes back.
- [ ] **Reduce effects** turns off glitch text, scanlines and glow (your OS's reduced-motion setting does too).
- [ ] **UI scale** from 90 to 150 %.

## Part 3 · Sounds

- [ ] **Settings → SOUND** → **UI sounds** is the master switch. Short synthesized blips play when an alert fires or clears (sharper for critical), a notice reaches the **INBOX**, something you started works or fails, a confirm dialog opens, DRIFT finishes a reply, a quest step is done, or a scan, test restore, drill or speed test finishes. Never for terminal output or live numbers.
- [ ] **VOLUME** starts at 40 %. **Mute when the window isn't focused** is on to start; **Quiet hours** with **FROM** and **UNTIL** silences a window every day.
- [ ] **SOUNDS** lists each kind (Alerts, Inbox, Actions, Dialogs, DRIFT, Quests, Long tasks, View switch) with its own switch and ▶ buttons to hear each sound. **View switch**, a soft tick when you change views, is off until you turn it on.
- [ ] The same **UI sounds** switch is also under Settings → **FEATURES**.

## Part 4 · Layout

- [ ] Overview dashboards: [Build dashboards with command widgets](kb:howto/command-widgets).
- [ ] Hide views you don't use: [Keep Nightgrid simple with feature presets](kb:howto/features).

#### Check

- [ ] `Mod+K`, type a host's name, Enter: its terminal opens.

## Limits and gotchas

- Your palette, sound choices and other settings travel in profile exports, so a new computer looks and sounds the same after import.

## Related

- [Keep Nightgrid simple with feature presets](kb:howto/features)
- [Build dashboards with command widgets](kb:howto/command-widgets)
