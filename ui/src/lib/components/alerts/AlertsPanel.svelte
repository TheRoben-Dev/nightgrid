<script lang="ts">
  import { healthIgnores } from "../../stores/healthIgnores.svelte";
  import { onMount } from "svelte";
  import Modal from "../Modal.svelte";
  import { scramble } from "../../scramble";
  import { api, errorText, type Alert, type LogNote } from "../../api";
  import { ago, alerts, customSecs, DURATIONS, groupByCause, isLogAlert, KIND_LABEL, logFingerprint, logSource, loud, scopeLabel, silenceWhen, targetLabel, UNITS } from "../../stores/alerts.svelte";
  import type { AckFor, MuteScope, Silence } from "../../api";
  import { factsFrom, logTriage, type ContainerFacts } from "../../helper/logTriage";
  import { triageCheck } from "../../helper/skills";
  import { helper } from "../../stores/helper.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { notes, runbookMatches } from "../../stores/notes.svelte";
  import { starters } from "../../stores/starters.svelte";
  import { startersFor } from "../../starters";

  const runbooks = (a: Alert) => notes.notes.filter((n) => runbookMatches(n, a));
  $effect(() => {
    if (alerts.open && !notes.loaded) void notes.load();
  });
  let pick = $state<number | null>(null);

  function openNote(id: number) {
    notes.draft = null;
    notes.selected = id;
    alerts.open = false;
    pick = null;
    ui.go("notes");
  }

  /** A new runbook for this kind of alert, on this host or service, with a step list to fill in. */
  /** Opens Knowledge → Starters on one starter runbook (D-177). */
  function openStarter(id: string) {
    notes.cancel();
    starters.focus = id;
    notes.starters = true;
    alerts.open = false;
    ui.go("notes");
  }

  function newRunbook(a: Alert) {
    const svc = /^svc:(\d+)/.exec(a.key);
    const serviceId = svc ? Number(svc[1]) : null;
    const hostId = serviceId === null ? a.hostId : null;
    const where = hosts.hosts.find((h) => h.id === hostId)?.name;
    notes.draft = {
      id: null,
      input: {
        title: `${KIND_LABEL[a.kind][0]}${KIND_LABEL[a.kind].slice(1).toLowerCase()}${serviceId !== null ? `: ${a.title}` : where ? ` on ${where}` : ""}`,
        body: ["When this alert fires:", "", "- [ ] Check …", "- [ ] Fix …", "- [ ] Confirm it cleared in [the alerts list](view:overview)", "", "```sh", "# a command to type into the terminal", "```"].join("\n"),
        hostId,
        tags: ["runbook"],
        pinned: false,
        hideFromDrift: false,
        runbook: true,
        alertKinds: [a.kind],
        serviceId,
      },
    };
    alerts.open = false;
    ui.go("notes");
  }

  /**
   * Triage. Log alerts and notes go to DRIFT with their events judged by the
   * log rules, the container's real state and a verdict (no guesswork);
   * other alerts with their usual causes and first checks.
   */
  async function triage(a: Alert) {
    if (isLogAlert(a)) {
      const lines = await alerts.samples(a).catch(() => [] as string[]);
      await triageLog({ title: a.title, hostId: a.hostId, source: logSource(a) ?? a.title, text: [`${KIND_LABEL[a.kind]}: ${a.title}`, a.detail, "", ...lines].join("\n"), lines: lines.join("\n") });
      return;
    }
    const host = hosts.hosts.find((h) => h.id === a.hostId)?.name ?? null;
    const text = [`${KIND_LABEL[a.kind]}: ${a.title}`, a.detail, `Since: ${new Date(a.since * 1000).toLocaleString()}`].join("\n");
    alerts.open = false;
    const mine = runbooks(a);
    const check = triageCheck(a.kind, a.title);
    if (mine.length) check.items.push({ text: `Your runbook${mine.length > 1 ? "s" : ""} for this: ${mine.map((n) => `"${n.title}"`).join(", ")} (in Knowledge; follow it first)` });
    void helper.explain({ source: `Alert: ${a.title}`, text, question: "Triage this alert: what is the likely cause, what should I check first, and where?", host, checks: [check] });
  }

  async function triageLog(o: { title: string; hostId: number | null; source: string; text: string; lines: string }) {
    const host = hosts.hosts.find((h) => h.id === o.hostId)?.name ?? null;
    const events = await api.logClassify(o.lines).catch(() => []);
    const [kind, name] = o.source.split("/", 2);
    let container: ContainerFacts | null = null;
    if (kind === "container" && name && o.hostId != null) {
      try {
        const d = await api.dockerInspect(o.hostId, name);
        if (!healthIgnores.loaded) await healthIgnores.load();
        container = factsFrom(name, d, null, healthIgnores.ignoredDetail(o.hostId, d));
      } catch (e) {
        container = factsFrom(name, null, errorText(e));
      }
    }
    const t = logTriage({ title: o.title, source: o.source, events, container, now: Math.floor(Date.now() / 1000) });
    alerts.open = false;
    const n = o.lines ? o.lines.split("\n").length : 0;
    void helper.explain({ source: `Log lines: ${o.source} · ${n} line${n === 1 ? "" : "s"}`, text: o.text, question: t.question, host, checks: [t.check] });
  }

  const noteTitle = (n: LogNote) => `${n.source.split("/").pop()} on ${hosts.hosts.find((h) => h.id === n.hostId)?.name ?? "host"}`;
  const WHY: Record<LogNote["why"], string> = { once: "seen once, below the repeat threshold", grace: "right after a restart (grace period)" };
  let muteError = $state<string | null>(null);
  async function mute(hostId: number, source: string, fp: string) {
    muteError = null;
    try {
      await alerts.mutePattern(hostId, source, fp);
    } catch (e) {
      muteError = errorText(e);
    }
  }

  let now = $state(Math.floor(Date.now() / 1000));
  onMount(() => {
    void starters.load();
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 15_000);
    return () => clearInterval(t);
  });
  const snap = $derived(alerts.snap);
  const loudActive = $derived(loud(alerts.active));
  const groups = $derived(groupByCause(loudActive));
  const hushedCount = $derived(alerts.active.length - loudActive.length);
  let openFold = $state<number | null>(null);

  // Acknowledge and mute (D-198).
  let menu = $state<{ id: number; what: "ack" | "mute" } | null>(null);
  let scope = $state<MuteScope["type"]>("alert");
  let customN = $state(2);
  let customUnit = $state<number>(3600);
  let silenceError = $state<string | null>(null);
  const hostName = (a: Alert) => hosts.hosts.find((h) => h.id === a.hostId)?.name ?? null;
  const custom = $derived(customSecs(customN, customUnit));

  function openMenu(a: Alert, what: "ack" | "mute") {
    silenceError = null;
    if (menu?.id === a.id && menu.what === what) {
      menu = null;
      return;
    }
    menu = { id: a.id, what };
    scope = "alert";
  }

  async function run(f: () => Promise<void>) {
    silenceError = null;
    try {
      await f();
      menu = null;
    } catch (e) {
      silenceError = errorText(e);
    }
  }
  const ack = (a: Alert, how: AckFor) => run(() => alerts.ack(a, how));
  const muteAs = (a: Alert, secs: number | null) => run(() => alerts.mute(a, { type: scope } as MuteScope, secs));
  function coverText(s: Silence): string {
    const hit = alerts.covered(s);
    if (hit.length === 1 && s.key) return hit[0].detail;
    if (hit.length) return `covers ${hit.length} active alert${hit.length === 1 ? "" : "s"}`;
    return "nothing active right now";
  }
  const SCOPES: MuteScope["type"][] = ["alert", "kindOnHost", "host", "kind"];
</script>

<Modal open={alerts.open} title="ALERTS" width={680} onclose={() => (alerts.open = false)}>
  <div class="head">
    <span class="count {alerts.tone}">{loudActive.length ? `${groups.length} ACTIVE` : "ALL CLEAR"}</span>
    {#if loudActive.length > groups.length}<span class="muted">+{loudActive.length - groups.length} folded under their cause</span>{/if}
    {#if hushedCount}<span class="muted">{hushedCount} acknowledged or muted</span>{/if}
    {#if snap && !snap.enabled}<span class="muted">Alerts are turned off.</span>{/if}
    {#if snap?.quiet}<span class="quiet">QUIET HOURS · notifications held</span>{/if}
  </div>

  {#if loudActive.length}
    <ul class="list" aria-label="Active alerts">
      {#each groups as { alert: a, folded } (a.key)}
        {@const rb = runbooks(a)}
        <li class="row {a.severity}">
          <span class="bar" aria-hidden="true"></span>
          <div class="txt">
            <span class="kind"><span use:scramble={KIND_LABEL[a.kind]}></span> · {a.title}</span>
            <span class="detail">{a.detail}</span>
          </div>
          <span class="when" title="Since {new Date(a.since * 1000).toLocaleString()}">{ago(a.since, now)}</span>
          {#if helper.enabled}
            <button type="button" class="btn ghost tiny" data-guide="alert-triage" title="Ask DRIFT for the likely cause and first checks (stays on this computer)" disabled={helper.busy} onclick={() => void triage(a)}
              ><span aria-hidden="true">✦ </span>{isLogAlert(a) ? "What broke?" : "Triage"}</button
            >
          {/if}
          {#if rb.length === 1}
            <button type="button" class="btn tiny" data-guide="alert-runbook" title="Open the runbook “{rb[0].title}”" onclick={() => openNote(rb[0].id)}>▤ Runbook</button>
          {:else if rb.length > 1}
            <button type="button" class="btn tiny" data-guide="alert-runbook" aria-expanded={pick === a.id} onclick={() => (pick = pick === a.id ? null : a.id)}>▤ Runbooks ({rb.length})</button>
          {:else}
            {@const st = startersFor(a.kind, starters.catalog?.runbooks ?? [], notes.notes)}
            {#if st.length}
              <button type="button" class="btn ghost tiny" title="Read the starter runbook “{st[0].title}” and add it to your notes" onclick={() => openStarter(st[0].id)}>▤ Starter runbook</button>
            {:else}
              <button type="button" class="btn ghost tiny" title="Write a runbook for this kind of alert" onclick={() => newRunbook(a)}>+ Runbook</button>
            {/if}
          {/if}
          {#if logFingerprint(a) && a.hostId != null}
            <button type="button" class="btn ghost tiny" data-guide="alert-mute" title="Never alert on this pattern from {logSource(a)} again (it's still counted)" onclick={() => void mute(a.hostId ?? 0, logSource(a) ?? "", logFingerprint(a) ?? "")}>Mute pattern</button>
          {/if}
          <button
            type="button"
            class="btn ghost tiny"
            data-guide="alert-ack"
            aria-expanded={menu?.id === a.id && menu.what === "ack"}
            title="I know about this: no badge or notifications until it gets worse"
            onclick={() => openMenu(a, "ack")}>✓ Ack</button
          >
          <button
            type="button"
            class="btn ghost tiny"
            data-guide="alert-ack"
            aria-expanded={menu?.id === a.id && menu.what === "mute"}
            title="Silence this alert, its kind or its host for a while or for good"
            onclick={() => openMenu(a, "mute")}>Mute…</button
          >
          <button type="button" class="btn ghost tiny" onclick={() => alerts.go(a)}>{targetLabel(a)}</button>
          {#if menu?.id === a.id}
            <div class="fold silence" role="group" aria-label={menu.what === "ack" ? "Acknowledge for how long" : "Mute what, and for how long"}>
              {#if menu.what === "ack"}
                <span class="muted small">Acknowledge: stays in the list, without a badge or notifications. It comes back if it gets worse ({a.severity === "warn" ? "WARN → CRITICAL" : "it's already critical, so only when it clears"}).</span>
                <div class="opts">
                  <button type="button" class="btn tiny" onclick={() => void ack(a, { type: "clear" })}>Until it clears</button>
                  {#each DURATIONS as d (d.secs)}<button type="button" class="btn ghost tiny" onclick={() => void ack(a, { type: "for", secs: d.secs })}>{d.label}</button>{/each}
                  <button type="button" class="btn ghost tiny" title="Only a worse state brings it back, even after it clears and returns" onclick={() => void ack(a, { type: "always" })}>Permanently</button>
                </div>
              {:else}
                <span class="muted small">Mute: no badge or notifications, whatever the severity, until you undo it or the time runs out.</span>
                <div class="opts">
                  {#each SCOPES as s (s)}
                    <label class="scope"
                      ><input type="radio" name="mute-scope-{a.id}" value={s} bind:group={scope} disabled={(s === "kindOnHost" || s === "host") && a.hostId == null} />
                      {scopeLabel(s, a, hostName(a))}</label
                    >
                  {/each}
                </div>
                <div class="opts">
                  {#each DURATIONS as d (d.secs)}<button type="button" class="btn ghost tiny" onclick={() => void muteAs(a, d.secs)}>{d.label}</button>{/each}
                  <button type="button" class="btn tiny" onclick={() => void muteAs(a, null)}>Forever</button>
                </div>
              {/if}
              <div class="opts">
                <span class="muted small">Custom:</span>
                <input class="input num" type="number" min="1" max="999" bind:value={customN} aria-label="How many" />
                <select class="input" bind:value={customUnit} aria-label="Unit">
                  {#each UNITS as u (u.secs)}<option value={u.secs}>{u.label}</option>{/each}
                </select>
                <button
                  type="button"
                  class="btn ghost tiny"
                  disabled={custom == null}
                  title={custom == null ? "Between a minute and a year" : ""}
                  onclick={() => custom != null && void (menu?.what === "ack" ? ack(a, { type: "for", secs: custom }) : muteAs(a, custom))}
                  >{menu.what === "ack" ? "Acknowledge" : "Mute"}</button
                >
              </div>
            </div>
          {/if}
          {#if pick === a.id && rb.length > 1}
            <div class="fold">
              {#each rb as n (n.id)}<button type="button" class="link" onclick={() => openNote(n.id)}>▤ {n.title}</button>{/each}
            </div>
          {/if}
          {#if folded.length}
            <div class="fold">
              <button type="button" class="link" aria-expanded={openFold === a.id} onclick={() => (openFold = openFold === a.id ? null : a.id)}
                >{openFold === a.id ? "▾" : "▸"} explains {folded.length} more alert{folded.length === 1 ? "" : "s"} (not notified separately)</button
              >
              {#if openFold === a.id}
                <ul aria-label="Alerts caused by {a.title}">
                  {#each folded as f (f.key)}
                    <li>
                      <span class="kind">{KIND_LABEL[f.kind]} · {f.title}</span>
                      <span class="detail">{f.detail}</span>
                      <span class="facts">
                        {#each runbooks(f).slice(0, 1) as n (n.id)}<button type="button" class="btn tiny" data-guide="alert-runbook" title="Open the runbook “{n.title}”" onclick={() => openNote(n.id)}>▤ Runbook</button>{/each}
                        <button type="button" class="btn ghost tiny" onclick={() => alerts.go(f)}>{targetLabel(f)}</button>
                      </span>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted clear">Nothing needs attention. Hosts, disks, containers, services, certificates, SMART and backups are watched in the background (and logs, if turned on).</p>
  {/if}

  {#if muteError}<p class="err" role="alert">{muteError}</p>{/if}
  {#if silenceError}<p class="err" role="alert">{silenceError}</p>{/if}

  {#if alerts.silences.length}
    <div class="sub">
      <span class="label">ACKNOWLEDGED & MUTED</span>
      <span class="muted small sublead">no badge, sound or notifications; still on the timeline</span>
    </div>
    <ul class="list past" aria-label="Acknowledged and muted">
      {#each alerts.silences as s (s.id)}
        <li class="row hushed {alerts.covered(s)[0]?.severity ?? ''}">
          <span class="bar" aria-hidden="true"></span>
          <div class="txt">
            <span class="kind">{s.mode === "ack" ? "ACK" : "MUTED"} · {s.label}</span>
            <span class="detail">{silenceWhen(s, now)} · {coverText(s)}</span>
          </div>
          <button type="button" class="btn ghost tiny" title={s.mode === "ack" ? "Count and notify this alert again" : "Remove this mute"} onclick={() => void run(() => alerts.unsilence(s.id))}>Undo</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if alerts.notes.length}
    <div class="sub">
      <span class="label">NOTED, NOT ALERTED</span>
      <span class="muted small sublead">new log errors that didn't meet the alert rules</span>
    </div>
    <ul class="list past" aria-label="Noted log errors">
      {#each alerts.notes.slice(0, 20) as n (`${n.hostId}|${n.source}|${n.fingerprint}`)}
        <li class="row note">
          <span class="bar" aria-hidden="true"></span>
          <div class="txt">
            <span class="kind">NOTE · {noteTitle(n)} · {WHY[n.why]}</span>
            <span class="detail">{n.sample?.split("\n")[0] ?? "(a line that looked like it held a secret)"}</span>
          </div>
          <span class="when">{ago(n.at, now)} ago</span>
          {#if helper.enabled && n.sample}
            <button type="button" class="btn ghost tiny" disabled={helper.busy} onclick={() => void triageLog({ title: noteTitle(n), hostId: n.hostId, source: n.source, text: n.sample ?? "", lines: n.sample ?? "" })}
              ><span aria-hidden="true">✦ </span>What is it?</button
            >
          {/if}
          <button type="button" class="btn ghost tiny" title="Don't note this pattern from {n.source} again" onclick={() => void mute(n.hostId, n.source, n.fingerprint)}>Mute pattern</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if alerts.quietLogs.length}
    <p class="muted small quietlogs" data-guide="alert-log-quiet">
      Left out on purpose:
      {#each alerts.quietLogs as q, i (`${q.hostId}|${q.source}`)}{i ? " · " : ""}{q.source.split("/").pop()}{q.muted ? ` ${q.muted} muted` : ""}{q.notFound ? ` ${q.notFound} not-found (404)` : ""}{/each}
      <span>(since the app started; mutes are in Settings → Alerts).</span>
    </p>
  {/if}

  {#if alerts.history.length}
    <div class="sub">
      <span class="label">RESOLVED</span>
      <span class="spacer"></span>
      <button type="button" class="btn ghost tiny" onclick={() => alerts.clearHistory()}>Clear</button>
    </div>
    <ul class="list past" aria-label="Resolved alerts">
      {#each alerts.history.slice(0, 50) as a (a.id)}
        <li class="row done">
          <span class="bar" aria-hidden="true"></span>
          <div class="txt">
            <span class="kind">{KIND_LABEL[a.kind]} · {a.title}</span>
            <span class="detail">{a.detail}</span>
          </div>
          <span class="when" title="Resolved {a.resolved ? new Date(a.resolved * 1000).toLocaleString() : ''}"
            >{a.resolved ? `${ago(a.resolved, now)} ago` : ""}</span
          >
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet footer()}
    <button
      type="button"
      class="btn ghost"
      onclick={() => {
        alerts.open = false;
        ui.settingsTab = "alerts";
        ui.go("settings");
      }}>Alert rules…</button
    >
    <button type="button" class="btn primary" onclick={() => (alerts.open = false)}>Close</button>
  {/snippet}
</Modal>

<style>
  .err {
    color: var(--danger);
    font-size: var(--fs-sm);
  }
  .silence {
    gap: 6px;
    padding: 6px 0 2px;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .scope {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
    margin-right: 8px;
  }
  .num {
    width: 64px;
  }
  .row.hushed {
    background: transparent;
    opacity: 0.75;
  }
  .row.hushed .bar {
    opacity: 0.5;
  }
  .row.note .bar {
    background: var(--cyan);
    opacity: 0.5;
  }
  .sublead {
    margin-left: 10px;
    letter-spacing: normal;
  }
  .quietlogs {
    margin: 8px 0 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .head {
    display: flex;
    gap: 12px;
    align-items: center;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    margin-bottom: 8px;
  }
  .count.ok {
    color: var(--ok);
  }
  .count.warn {
    color: var(--warn);
  }
  .count.bad {
    color: var(--danger);
  }
  .quiet {
    color: var(--mint);
  }
  .clear {
    margin: 8px 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    max-height: 320px;
    overflow: auto;
  }
  .past {
    max-height: 200px;
  }
  .fold {
    grid-column: 2 / -1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .fold ul {
    list-style: none;
    margin: 0;
    padding: 0 0 0 12px;
    border-left: 1px solid var(--line);
    display: grid;
    gap: 4px;
  }
  .fold li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 8px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .fold li .detail {
    grid-column: 1;
    color: var(--text-1);
  }
  .fold li .facts {
    grid-row: 1 / span 2;
    grid-column: 2;
    display: flex;
    gap: 6px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-xs);
    text-align: left;
  }
  .row {
    --tone: var(--text-1);
    display: grid;
    grid-template-columns: 2px 1fr repeat(7, auto);
    gap: 10px;
    align-items: center;
    padding: 6px 8px 6px 0;
    border: 1px solid var(--line);
    background: var(--bg-1);
  }
  .row.warn {
    --tone: var(--warn);
  }
  .row.critical {
    --tone: var(--danger);
  }
  .row.done {
    --tone: var(--mint);
    background: transparent;
  }
  .bar {
    align-self: stretch;
    background: var(--tone);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .kind {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--tone);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done .kind {
    color: var(--text-1);
  }
  .detail {
    font-size: var(--fs-sm);
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done .detail {
    color: var(--text-1);
  }
  .when {
    font-size: var(--fs-xs);
    color: var(--text-1);
    font-family: var(--font-mono);
  }
  .sub {
    display: flex;
    align-items: center;
    margin: 14px 0 6px;
  }
  .spacer {
    flex: 1;
  }
</style>
