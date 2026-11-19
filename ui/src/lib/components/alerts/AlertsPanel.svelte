<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../Modal.svelte";
  import { scramble } from "../../scramble";
  import { api, errorText, type Alert, type LogNote } from "../../api";
  import { ago, alerts, groupByCause, isLogAlert, KIND_LABEL, logFingerprint, logSource, targetLabel } from "../../stores/alerts.svelte";
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
        container = factsFrom(name, await api.dockerInspect(o.hostId, name), null);
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
      await alerts.mute(hostId, source, fp);
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
  const groups = $derived(groupByCause(alerts.active));
  let openFold = $state<number | null>(null);
</script>

<Modal open={alerts.open} title="ALERTS" width={680} onclose={() => (alerts.open = false)}>
  <div class="head">
    <span class="count {alerts.tone}">{alerts.active.length ? `${groups.length} ACTIVE` : "ALL CLEAR"}</span>
    {#if alerts.active.length > groups.length}<span class="muted">+{alerts.active.length - groups.length} folded under their cause</span>{/if}
    {#if snap && !snap.enabled}<span class="muted">Alerts are turned off.</span>{/if}
    {#if snap?.quiet}<span class="quiet">QUIET HOURS · notifications held</span>{/if}
  </div>

  {#if alerts.active.length}
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
          <button type="button" class="btn ghost tiny" onclick={() => alerts.go(a)}>{targetLabel(a)}</button>
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
    grid-template-columns: 2px 1fr repeat(4, auto);
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
