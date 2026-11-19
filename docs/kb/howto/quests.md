# Follow a guide as a quest

Turn any Library guide with steps into a quest tied to one host: Nightgrid keeps your place, shows the step you're on in the quest log, and ticks steps it can check on the host by itself.

## What you end up with

- A guide you can leave and come back to days later, with the step you're on remembered for that host
- The same guide running as separate quests on several hosts (one per server you set up)
- Steps that Nightgrid checks on the host, read-only, ticking themselves when they're done

## Before you start

- A saved host the guide is for, or none: a guide can also run with **No host**, for example a Linux skill you're practising.

## Part 1 · Find a quest

- [ ] Open the [Library](view:library) (`Mod+Shift+L`). Articles marked **◇ QUEST** have steps to tick and can be started as a quest.
- [ ] The **◇ QUESTS** chip above the list shows only those. It combines with the shelf chips, the topic list and search, so "Self-hosted apps + Media + ◇ QUESTS" lists app guides you can follow.

## Part 2 · Start it

- [ ] Open the guide. Under the title, **◇ QUEST AVAILABLE** has a host list: pick the server you're setting up, or **No host**.
- [ ] Press **▶ START QUEST**. The strip changes to **◆ ACTIVE QUEST**, with **QUEST LOG** and **ABANDON** beside it.
- [ ] Your first quest on a guide starts with the ticks you already had in it. Later quests on other hosts start empty.

## Part 3 · Work through it

- [ ] Tick steps as you do them. A quest update shows in the top right corner: **QUEST UPDATED** for a step, **OBJECTIVE COMPLETE** when a part is done, **QUEST COMPLETE** at the end. Click it to go back to the guide.
- [ ] The **OBJECTIVES** box beside the article lists its parts with how many steps you've ticked in each.
- [ ] Guides with checkable steps show **VERIFY ON** with the quest's host. **Check** runs the checks: each step gets ✓ (done on that host), ✗ (not yet, with what was found) or ? (couldn't tell). On a quest, passed steps tick themselves; without one, **Tick verified** ticks them.
- [ ] While Nightgrid is connected to the host anyway, it checks again every few minutes, so a step you did in the terminal ticks itself. It never connects or asks for a password just for this.

## Part 4 · The quest log

- [ ] **◆ QUESTS** in the top bar, beside **INBOX**, opens the quest log: every active quest with its host, the current objective, progress and the step you're on. The number on the button counts active quests.
- [ ] Click a quest to open its guide on that host. **Check now** runs its checks; **Abandon** stops tracking it (the guide stays in the Library).
- [ ] Finished quests move under **Completed**.

#### Check

- [ ] The quest log lists your quest with the right host and the step you're on.

## Limits and gotchas

- Checks only read: no sudo, nothing changed, and the results aren't stored. A step whose check can't tell (for example, a file only root can read) shows ? and stays for you to tick.
- Checks only ever tick steps; they never untick one.
- Quests live in Nightgrid's database and travel in profile exports. Ticks on articles you read without a quest stay on this computer only. Once every step is ticked, **RESET** in the **OBJECTIVES** box clears them.
- Quest sounds (**Quest updated**, **Quest complete**) can be turned off under **Settings → SOUND** (see [Shortcuts, palette and themes](kb:howto/look-and-feel)).

## Related

- [Raise your fleet's security score](kb:guides/security-posture)
- [Start from a ready-made setup](kb:howto/starters)
- [Get the most out of DRIFT](kb:howto/drift)
