# Build dashboards with command widgets

Turn any read-only command into an Overview tile with thresholds, and arrange several dashboards for different jobs.

## What you end up with

- Tiles for numbers Nightgrid doesn't track by itself: queue lengths, active streams, certificate counts, pod counts
- Green, orange and red by your thresholds
- Named dashboards ("Media", "Lab") with their own layouts

## Part 1 · Edit the Overview

- [ ] [Overview](view:overview) → **Edit**. Drag title bars to move, the bottom-right corner to resize, **×** to remove, **+ Widget** to add.
- [ ] Keyboard: Tab to a widget's **⠿** grip, arrows move, Shift+arrows resize.
- [ ] **+ New** makes another dashboard (empty or from a preset); **Rename**, **Delete**, **Presets**, and **Save this layout as a preset**.
- [ ] **Done** when finished. Changes save as you go; **Undo** and **Reset** are there.

## Part 2 · Add a command tile

- [ ] **+ Widget → COMMAND…**: pick the host and type the command. **START FROM** fills in common ones.
- [ ] Choose how to read the value: first number, a regex group (`(\d+)%`), line count, exit code, or the first line as text.
- [ ] Unit, warn and critical thresholds (tick **Low is bad** when lower is worse), refresh interval.
- [ ] **Try it** runs it once so you see the value before saving.

Ideas that work well:

```sh
docker ps -q | wc -l
```

```sh
df --output=pcent /srv | tail -1
```

```sh
systemctl is-active nginx
```

```sh
k3s kubectl get pods -A --no-headers | grep -vc Running
```

```sh
cat /sys/class/thermal/thermal_zone0/temp
```

#### Check

- [ ] The tile shows a value and turns orange when you lower the warn threshold below it.

## Part 3 · Manage tiles

- [ ] **Output** shows what the command printed; **Edit…** changes it.
- [ ] A removed tile stays under **YOUR COMMANDS** in the add menu; **Delete…** in the editor removes it everywhere.

## Limits and gotchas

- Commands that change something, and sudo, are refused because tiles refresh unattended.
- Keep commands fast. A slow command on a short interval keeps an SSH channel busy.
- Dashboards and presets travel in profile exports.

## Related

- [Get the most out of DRIFT](kb:howto/drift)
- [Kubernetes-lite with k3s](kb:apps/k3s)
- [Prometheus and Grafana](kb:apps/prometheus-grafana)
