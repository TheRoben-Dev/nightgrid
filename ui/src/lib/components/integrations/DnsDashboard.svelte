<script lang="ts">
  /**
   * Pi-hole / AdGuard Home dashboard (M10v): stats, pause blocking, block
   * and allow lists, local DNS records. Turning blocking off and removals
   * are confirmed with the target named (rule 5).
   */
  import { untrack } from "svelte";
  import type { DnsList, DnsRecord } from "../../api";
  import { duration, isIp, listName, PAUSES, sortLists, validRecordDomain } from "../../dnsfilter";
  import { integrations, KIND_LABEL } from "../../stores/integrations.svelte";
  import Modal from "../Modal.svelte";

  const id = $derived(integrations.dnsOpen);
  const view = $derived(id !== null ? integrations.byId(id) : undefined);
  const i = $derived(view?.integration);
  const entry = $derived(id !== null ? integrations.entries[id] : undefined);
  const s = $derived(entry?.status && (entry.status.kind === "pihole" || entry.status.kind === "adguard") ? entry.status : null);
  const adguard = $derived(i?.kind === "adguard");
  const busy = $derived(integrations.dnsBusy);

  let pause = $state<string>("300");
  let confirm = $state<{ text: string; button: string; run: () => Promise<boolean> } | null>(null);
  let listUrl = $state("");
  let listName_ = $state("");
  let listAllow = $state(false);
  let recDomain = $state("");
  let recAnswer = $state("");

  $effect(() => {
    const open = id;
    if (open === null) return;
    untrack(() => {
      confirm = null;
      void integrations.refresh(open);
    });
  });

  const recOk = $derived(validRecordDomain(recDomain.trim(), adguard) && (isIp(recAnswer.trim()) || (adguard && validRecordDomain(recAnswer.trim(), false))));
  const listOk = $derived(/^https?:\/\/\S+$/.test(listUrl.trim()));

  function run(action: Parameters<typeof integrations.dns>[1], done: string) {
    if (id === null) return Promise.resolve(false);
    return integrations.dns(id, action, done);
  }

  function askPause() {
    if (!i) return;
    const secs = pause === "" ? null : Number(pause);
    const what = secs ? `for ${duration(secs)}` : "until you turn it back on";
    confirm = {
      text: `Turn ad blocking off on ${i.name} ${what}? Every device using it gets ads and trackers again.`,
      button: secs ? `Pause ${i.name} for ${duration(secs)}` : `Turn ${i.name} off`,
      run: () => run({ op: "setBlocking", enabled: false, seconds: secs }, secs ? `Blocking paused for ${duration(secs)}.` : "Blocking is off."),
    };
  }
  function askRemoveList(l: DnsList) {
    confirm = {
      text: `Remove the ${l.allow ? "allow" : "block"} list ${listName(l)} (${l.url}) from ${i?.name}?`,
      button: `Remove ${listName(l)}`,
      run: () => run({ op: "removeList", url: l.url, allow: l.allow }, `Removed ${listName(l)}.`),
    };
  }
  function askRemoveRecord(r: DnsRecord) {
    confirm = {
      text: `Remove the record ${r.domain} → ${r.answer} from ${i?.name}? Devices stop finding ${r.domain} through it.`,
      button: `Remove ${r.domain}`,
      run: () => run({ op: "removeRecord", domain: r.domain, answer: r.answer }, `Removed ${r.domain}.`),
    };
  }
  async function confirmed() {
    const c = confirm;
    confirm = null;
    if (c) await c.run();
  }
  async function addList(e: Event) {
    e.preventDefault();
    if (await run({ op: "addList", url: listUrl.trim(), name: listName_.trim(), allow: listAllow }, `Added ${listName_.trim() || listUrl.trim()}. Update lists to download it.`)) {
      listUrl = "";
      listName_ = "";
    }
  }
  async function addRecord(e: Event) {
    e.preventDefault();
    if (await run({ op: "addRecord", domain: recDomain.trim(), answer: recAnswer.trim() }, `${recDomain.trim()} now answers ${recAnswer.trim()}.`)) {
      recDomain = "";
      recAnswer = "";
    }
  }
  const ago = (t: number | null) => {
    if (!t) return "never";
    const d = Date.now() / 1000 - t;
    return d < 3600 ? `${Math.max(1, Math.round(d / 60))} min ago` : d < 86400 * 2 ? `${Math.round(d / 3600)} h ago` : `${Math.round(d / 86400)} d ago`;
  };
</script>

<Modal open={id !== null} title={i ? `${KIND_LABEL[i.kind]} :: ${i.name}` : "DNS"} width={880} onclose={() => (integrations.dnsOpen = null)}>
  <div class="dash" data-guide="dns-dashboard">
    {#if entry?.error}<p class="bad small" role="alert">{entry.error}</p>{/if}
    {#if !s}
      <p class="muted small">{entry?.loading ? "Reading…" : ""}</p>
    {:else}
      <div class="stats">
        <div><b>{s.queries.toLocaleString("en-US")}</b><span>QUERIES</span></div>
        <div><b>{s.blocked.toLocaleString("en-US")}</b><span>BLOCKED</span></div>
        <div><b>{(s.percentBlocked ?? 0).toFixed(1)}%</b><span>BLOCKED SHARE</span></div>
        {#if s.clients != null}<div><b>{s.clients}</b><span>{adguard ? "TOP CLIENTS" : "ACTIVE CLIENTS"}</span></div>{/if}
        {#if s.onLists != null}<div><b>{s.onLists.toLocaleString("en-US")}</b><span>{adguard ? "RULES" : "DOMAINS ON LISTS"}</span></div>{/if}
      </div>

      <div class="blocking" class:off={!s.blocking} data-guide="dns-blocking">
        {#if s.blocking}
          <span class="ok">● BLOCKING ON</span>
          <span class="spacer"></span>
          <select class="input" bind:value={pause} aria-label="Pause for">
            {#each PAUSES as p (p.label)}<option value={p.seconds === null ? "" : String(p.seconds)}>{p.label}</option>{/each}
          </select>
          <button type="button" class="btn ghost tiny" disabled={busy} onclick={askPause}>Pause…</button>
        {:else}
          <span class="warn">● BLOCKING {s.resumesIn ? `PAUSED · back on in ${duration(s.resumesIn)}` : "OFF"}</span>
          <span class="spacer"></span>
          <button type="button" class="btn primary tiny" disabled={busy} onclick={() => void run({ op: "setBlocking", enabled: true, seconds: null }, "Blocking is on again.")}>Turn on now</button>
        {/if}
      </div>

      {#if confirm}
        <div class="confirm" role="alertdialog" aria-label="Confirm">
          <p>{confirm.text}</p>
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => (confirm = null)}>Keep</button>
          <button type="button" class="btn danger tiny" disabled={busy} onclick={() => void confirmed()}>{confirm.button}</button>
        </div>
      {/if}

      <div class="cols">
        <section>
          <h4>TOP BLOCKED</h4>
          <ul class="rows">
            {#each s.topBlocked as d (d.domain)}
              <li><span class="main" title={d.domain}>{d.domain}</span><span class="meta">{d.count.toLocaleString("en-US")}</span></li>
            {:else}<li class="muted small">Nothing blocked yet.</li>{/each}
          </ul>
        </section>
        <section>
          <h4>LOCAL DNS {adguard ? "REWRITES" : "RECORDS"}</h4>
          <ul class="rows">
            {#each s.records as r (r.domain + r.answer)}
              <li>
                <span class="main">{r.domain}</span><code class="meta">{r.answer}</code>
                <button type="button" class="btn ghost tiny" disabled={busy} onclick={() => askRemoveRecord(r)}>Remove…</button>
              </li>
            {:else}<li class="muted small">No local records.</li>{/each}
          </ul>
          <form class="row" onsubmit={addRecord}>
            <input class="input grow" bind:value={recDomain} placeholder={adguard ? "nas.lan or *.apps.lan" : "nas.lan"} aria-label="Record domain" spellcheck="false" />
            <input class="input ip" bind:value={recAnswer} placeholder="192.168.1.10" aria-label="Record answer" spellcheck="false" />
            <button type="submit" class="btn primary tiny" data-guide="dns-add-record" disabled={busy || !recOk}>Add</button>
          </form>
        </section>
      </div>

      <section>
        <div class="row">
          <h4>LISTS</h4>
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" disabled={busy} onclick={() => void run({ op: "updateLists" }, "Lists downloaded again.")}>{busy ? "Working…" : adguard ? "Update lists" : "Update gravity"}</button>
        </div>
        <div class="lists" aria-label="Block and allow lists">
          {#each sortLists(s.lists) as l (l.url + l.allow)}
            <div class="list" class:dis={!l.enabled}>
              <input type="checkbox" checked={l.enabled} disabled={busy} title={l.enabled ? "Turn this list off" : "Turn this list on"} onchange={() => void run({ op: "setListEnabled", url: l.url, name: l.name, allow: l.allow, enabled: !l.enabled }, `${listName(l)} is ${l.enabled ? "off" : "on"}.`)} />
              <span class="tag" class:allow={l.allow}>{l.allow ? "ALLOW" : "BLOCK"}</span>
              <span class="lname" title={l.url}>{listName(l)}<code>{l.url}</code></span>
              <span class="meta">{l.entries != null ? l.entries.toLocaleString("en-US") : "—"}</span>
              <span class="meta">{ago(l.updated)}</span>
              <button type="button" class="btn ghost tiny" disabled={busy} onclick={() => askRemoveList(l)}>Remove…</button>
            </div>
          {:else}<p class="muted small pad">No lists.</p>{/each}
        </div>
        <form class="row" onsubmit={addList}>
          <input class="input grow" bind:value={listUrl} placeholder="https://example.com/blocklist.txt" aria-label="List URL" spellcheck="false" />
          <input class="input" bind:value={listName_} placeholder="name" aria-label="List name" />
          <label class="check"><input type="checkbox" bind:checked={listAllow} />allow list</label>
          <button type="submit" class="btn primary tiny" data-guide="dns-add-list" disabled={busy || !listOk}>Add list</button>
        </form>
      </section>
      <p class="muted small">{KIND_LABEL[i!.kind]}{s.version ? ` ${s.version}` : ""} · {i!.baseUrl}</p>
    {/if}
  </div>
  {#snippet footer()}
    <button type="button" class="btn ghost" disabled={entry?.loading} onclick={() => id !== null && void integrations.refresh(id)}>Refresh</button>
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={() => (integrations.dnsOpen = null)}>Close</button>
  {/snippet}
</Modal>

<style>
  .dash {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 8px;
  }
  .stats div {
    border: 1px solid var(--line);
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .stats b {
    font-family: var(--font-mono);
    font-size: var(--fs-lg, 18px);
    font-weight: 500;
    color: var(--text-0);
  }
  .stats span,
  h4 {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
    margin: 0;
    font-weight: 500;
  }
  h4 {
    color: var(--cyan);
    margin-bottom: 4px;
  }
  .blocking,
  .confirm,
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .blocking {
    border: 1px solid var(--line);
    padding: 6px 8px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .blocking.off {
    border-color: var(--warn, var(--yellow));
  }
  .confirm {
    border: 1px solid var(--danger);
    padding: 6px 8px;
    font-size: var(--fs-sm);
  }
  .confirm p {
    margin: 0;
    flex: 1 1 300px;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  @media (max-width: 760px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
  .rows {
    list-style: none;
    margin: 0 0 6px;
    padding: 0;
    border: 1px solid var(--line);
    max-height: 200px;
    overflow: auto;
  }
  .rows li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 3px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-xs);
  }
  .main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--text-1);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .lists {
    border: 1px solid var(--line);
    max-height: 240px;
    overflow: auto;
    margin-bottom: 6px;
  }
  .list {
    display: grid;
    grid-template-columns: 20px 52px minmax(0, 1fr) 80px 80px auto;
    gap: 8px;
    align-items: center;
    padding: 3px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-xs);
  }
  .list.dis {
    opacity: 0.55;
  }
  .lname {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .lname code {
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--danger);
  }
  .tag.allow {
    color: var(--mint);
  }
  .grow {
    flex: 1 1 200px;
  }
  .ip {
    width: 150px;
  }
  .check {
    display: flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 8px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn, var(--yellow));
  }
  .bad {
    color: var(--danger);
  }
  .muted {
    color: var(--text-1);
  }
</style>
