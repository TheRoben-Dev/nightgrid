<script lang="ts">
  /**
   * System → FIREWALL (M10v): ufw rules or firewalld zones with changes that
   * roll back on the host unless kept; nftables / iptables read-only.
   */
  import type { FwChange } from "../../api";
  import { firewall } from "../../stores/firewall.svelte";
  import { system } from "../../stores/system.svelte";
  import Modal from "../Modal.svelte";

  type Act = "allow" | "deny" | "reject" | "limit";
  type Pr = "tcp" | "udp" | "any";

  $effect(() => {
    if (system.hostId !== null) void firewall.load();
  });

  const v = $derived(firewall.view);
  const r = $derived(v?.report);
  const pending = $derived(v?.pending ?? null);

  // Countdown to the host's own rollback.
  let now = $state(Date.now() / 1000);
  $effect(() => {
    if (!pending) return;
    const t = setInterval(() => {
      now = Date.now() / 1000;
      if (pending && now >= pending.deadline) firewall.expired();
    }, 500);
    return () => clearInterval(t);
  });
  const left = $derived(pending ? Math.max(0, Math.ceil(pending.deadline - now)) : 0);

  // ufw add form.
  let action = $state<Act>("allow");
  let port = $state("");
  let proto = $state<Pr>("tcp");
  let from = $state("");
  let comment = $state("");
  // firewalld forms.
  let zone = $state("");
  let service = $state("");
  let fport = $state("");
  let fproto = $state<"tcp" | "udp">("tcp");
  let source = $state("");
  $effect(() => {
    if (r?.zones.length && !r.zones.some((z) => z.name === zone)) zone = r.zones[0].name;
  });

  // Confirmation.
  let secs = $state(120);
  let typed = $state("");
  let applyError = $state<string | null>(null);
  const hostName = $derived(system.host?.name ?? "");
  function propose(c: FwChange) {
    typed = "";
    applyError = null;
    void firewall.propose(c);
  }
  async function apply() {
    applyError = await firewall.apply(secs, firewall.proposal?.check.risk ? typed : null);
    if (!applyError) {
      wport = "";
      wfrom = "";
      wcomment = "";
      port = "";
      from = "";
      comment = "";
      service = "";
      fport = "";
      source = "";
    }
  }
  const BACKEND: Record<string, string> = { ufw: "ufw", firewalld: "firewalld", nftables: "nftables (read-only)", iptables: "iptables (read-only)", windows: "Windows Defender Firewall", none: "none found" };
  // Windows add form.
  let wport = $state("");
  let wproto = $state<"tcp" | "udp">("tcp");
  let wfrom = $state("");
  let wcomment = $state("");
  const winBlocks = $derived((r?.windows?.profiles ?? []).every((p) => p.defaultIn !== "Allow"));
  const sourceRule = (rule: string) => /^rule family="ipv[46]" source address="([^"]+)"(?: port port="([^"]+)" protocol="(tcp|udp)")? accept$/.exec(rule);
</script>

<div class="fw" data-guide="system-firewall">
  <div class="bar">
    {#if r}
      <span class="label">FIREWALL</span>
      <b>{BACKEND[r.backend]}</b>
      {#if r.backend === "ufw" || r.backend === "firewalld" || r.backend === "windows"}<span class="state" class:on={r.active}>{r.active ? "ACTIVE" : "INACTIVE"}</span>{/if}
      {#if r.ufw}<span class="muted small">incoming {r.ufw.defaultIn || "?"} · outgoing {r.ufw.defaultOut || "?"}</span>{/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" disabled={firewall.loading} onclick={() => void firewall.load()}>{firewall.loading ? "…" : "Refresh"}</button>
  </div>

  {#if pending}
    <div class="pending" role="status" data-guide="firewall-pending">
      <div>
        <b>Changed: {pending.change}.</b>
        The host puts the old rules back in <b class="count">{left}s</b> unless you keep it.
        {#if firewall.test}<p class="small {firewall.test.state === 'ok' ? 'ok' : firewall.test.state === 'failed' ? 'bad' : 'muted'}">{firewall.test.text}</p>{/if}
      </div>
      <div class="acts">
        <button type="button" class="btn ghost tiny" disabled={firewall.busy || firewall.test?.state === "running"} onclick={() => void firewall.runTest()}>Test again</button>
        <button type="button" class="btn ghost tiny" disabled={firewall.busy} onclick={() => void firewall.finish(false)}>Undo now</button>
        <button type="button" class="btn primary tiny" data-guide="firewall-keep" disabled={firewall.busy} onclick={() => void firewall.finish(true)}>Keep</button>
      </div>
    </div>
  {:else if r?.rollbackArmed}
    <p class="warn small">A rollback from an earlier change is still armed on this host; it will put that change back.</p>
  {/if}
  {#if firewall.notice}<p class="small notice">{firewall.notice}</p>{/if}
  {#if firewall.error}<p class="bad small" role="alert">{firewall.error}</p>{/if}
  {#if !r && firewall.loading}<p class="muted small">Reading the firewall…</p>{/if}

  {#if r}
    <div class="scroll">
      {#if r.backend === "ufw" && r.ufw}
        <section>
          <div class="row head">
            <h3 class="label">// UFW RULES</h3>
            <span class="spacer"></span>
            <label class="small">Incoming default
              <select class="input tiny" value={r.ufw.defaultIn} disabled={!!pending} onchange={(e) => propose({ op: "ufwDefault", policy: (e.currentTarget as HTMLSelectElement).value })}>
                <option value="deny">deny</option><option value="reject">reject</option><option value="allow">allow</option>
              </select>
            </label>
            <button type="button" class="btn tiny" disabled={!!pending} onclick={() => propose({ op: "ufwEnable", enabled: !r.active })}>{r.active ? "Turn off…" : "Turn on…"}</button>
          </div>
          {#each r.ufw.rules as rule (rule.spec)}
            <div class="rule">
              <span class="act {rule.action}">{rule.action.toUpperCase()}</span>
              <code>{rule.spec.replace(/\s*comment '.*'$/, "")}</code>
              <span class="muted small">{rule.comment ?? ""}</span>
              <button type="button" class="btn ghost tiny" disabled={!!pending} onclick={() => propose({ op: "ufwDelete", spec: rule.spec })}>Delete…</button>
            </div>
          {:else}
            <p class="muted small">No rules.</p>
          {/each}
          <form class="add" onsubmit={(e) => (e.preventDefault(), propose({ op: "ufwAdd", action, port: port.trim(), proto, from: from.trim() || null, comment: comment.trim() || null }))}>
            <select class="input" bind:value={action} aria-label="Action"><option value="allow">allow</option><option value="limit">limit</option><option value="deny">deny</option><option value="reject">reject</option></select>
            <input class="input port" bind:value={port} placeholder="port or 8000:8100" aria-label="Port" />
            <select class="input" bind:value={proto} aria-label="Protocol"><option value="tcp">tcp</option><option value="udp">udp</option><option value="any">any</option></select>
            <input class="input" bind:value={from} placeholder="from (any, 192.168.1.0/24)" aria-label="From" />
            <input class="input" bind:value={comment} placeholder="comment" aria-label="Comment" />
            <button type="submit" class="btn primary tiny" data-guide="firewall-add" disabled={!!pending || !port.trim()}>Add…</button>
          </form>
        </section>
      {:else if r.backend === "windows" && r.windows}
        {@const w = r.windows}
        <section>
          <div class="row head">
            <h3 class="label">// PROFILES</h3>
            <span class="spacer"></span>
            <label class="small">Incoming default
              <select class="input tiny" value={winBlocks ? "block" : "allow"} disabled={!!pending} onchange={(e) => propose({ op: "winDefault", policy: (e.currentTarget as HTMLSelectElement).value })}>
                <option value="block">block</option><option value="allow">allow</option>
              </select>
            </label>
            <button type="button" class="btn tiny" disabled={!!pending} onclick={() => propose({ op: "winEnable", enabled: !r.active })}>{r.active ? "Turn off…" : "Turn on…"}</button>
          </div>
          <p class="small">
            {#each w.profiles as p (p.name)}<span class="chip">{p.name}: {p.enabled ? "on" : "OFF"} · in {p.defaultIn === "NotConfigured" ? "block" : p.defaultIn.toLowerCase()}</span> {/each}
          </p>
        </section>
        <section>
          <h3 class="label">// YOUR INBOUND RULES</h3>
          {#each w.rules as rule (rule.name)}
            <div class="rule" class:off={!rule.enabled}>
              <span class="act {rule.action === 'Allow' ? 'allow' : 'deny'}">{rule.action.toUpperCase()}</span>
              <code>{rule.protocol} {rule.ports || "any"}{rule.remote && rule.remote !== "Any" ? ` from ${rule.remote}` : ""}</code>
              <span class="muted small" title={rule.program ?? ""}>{rule.display}{rule.enabled ? "" : " · disabled"}{rule.program ? " · one program" : ""}</span>
              <button type="button" class="btn ghost tiny" disabled={!!pending} onclick={() => propose({ op: "winRuleEnable", name: rule.name, enabled: !rule.enabled })}>{rule.enabled ? "Disable…" : "Enable…"}</button>
              <button type="button" class="btn ghost tiny" disabled={!!pending} onclick={() => propose({ op: "winDelete", name: rule.name })}>Delete…</button>
            </div>
          {:else}
            <p class="muted small">No rules of your own.</p>
          {/each}
          <p class="muted small">Plus {w.builtinAllow} enabled allow rules that Windows and installed apps ship (not listed; change them in Windows Defender Firewall).</p>
          <form class="add" onsubmit={(e) => (e.preventDefault(), propose({ op: "winAllow", port: wport.trim(), proto: wproto, from: wfrom.trim() || null, comment: wcomment.trim() || null }))}>
            <input class="input port" bind:value={wport} placeholder="port or 8000-8100" aria-label="Port" />
            <select class="input" bind:value={wproto} aria-label="Protocol"><option value="tcp">tcp</option><option value="udp">udp</option></select>
            <input class="input" bind:value={wfrom} placeholder="from (any, 192.168.1.0/24)" aria-label="From" />
            <input class="input" bind:value={wcomment} placeholder="name" aria-label="Name" />
            <button type="submit" class="btn primary tiny" disabled={!!pending || !wport.trim()}>Allow…</button>
          </form>
        </section>
      {:else if r.backend === "firewalld"}
        {#each r.zones as z (z.name)}
          <section>
            <div class="row head">
              <h3 class="label">// ZONE {z.name.toUpperCase()}</h3>
              {#if z.default}<span class="tag">DEFAULT</span>{/if}{#if z.active}<span class="tag ok">ACTIVE</span>{/if}
              <span class="muted small">target {z.target}{z.interfaces.length ? ` · ${z.interfaces.join(", ")}` : ""}{z.sources.length ? ` · from ${z.sources.join(", ")}` : ""}</span>
            </div>
            <p class="chips"><span class="k">SERVICES</span>
              {#each z.services as s (s)}<span class="chip">{s}<button type="button" title="Remove {s}" disabled={!!pending} onclick={() => propose({ op: "firewalldService", zone: z.name, service: s, add: false })}>×</button></span>{:else}<span class="muted small">none</span>{/each}
            </p>
            <p class="chips"><span class="k">PORTS</span>
              {#each z.ports as p (p)}
                {@const [pn, pp] = p.split("/")}
                <span class="chip">{p}<button type="button" title="Close {p}" disabled={!!pending || (pp !== "tcp" && pp !== "udp")} onclick={() => propose({ op: "firewalldPort", zone: z.name, port: pn, proto: pp as "tcp" | "udp", add: false })}>×</button></span>
              {:else}<span class="muted small">none</span>{/each}
            </p>
            {#if z.richRules.length}
              <p class="chips"><span class="k">RULES</span>
                {#each z.richRules as rr (rr)}
                  {@const m = sourceRule(rr)}
                  <span class="chip mono">{rr}{#if m}<button type="button" title="Remove" disabled={!!pending} onclick={() => propose({ op: "firewalldSource", zone: z.name, source: m[1], port: m[2] ?? null, proto: (m[3] as "tcp" | "udp") ?? "any", add: false })}>×</button>{/if}</span>
                {/each}
              </p>
            {/if}
          </section>
        {/each}
        <section class="add-fwd">
          <div class="row">
            <label class="small">Zone <select class="input" bind:value={zone}>{#each r.zones as z (z.name)}<option value={z.name}>{z.name}</option>{/each}</select></label>
          </div>
          <form class="add" onsubmit={(e) => (e.preventDefault(), propose({ op: "firewalldService", zone, service: service.trim(), add: true }))}>
            <input class="input" bind:value={service} placeholder="service (http, https, samba…)" aria-label="Service" />
            <button type="submit" class="btn tiny" disabled={!!pending || !service.trim()}>Add service…</button>
          </form>
          <form class="add" onsubmit={(e) => (e.preventDefault(), propose({ op: "firewalldPort", zone, port: fport.trim(), proto: fproto, add: true }))}>
            <input class="input port" bind:value={fport} placeholder="port or 8000-8100" aria-label="Port" />
            <select class="input" bind:value={fproto} aria-label="Protocol"><option value="tcp">tcp</option><option value="udp">udp</option></select>
            <button type="submit" class="btn tiny" disabled={!!pending || !fport.trim()}>Open port…</button>
          </form>
          <form class="add" onsubmit={(e) => (e.preventDefault(), propose({ op: "firewalldSource", zone, source: source.trim(), port: null, proto: "any", add: true }))}>
            <input class="input" bind:value={source} placeholder="accept everything from 192.168.1.0/24" aria-label="Source" />
            <button type="submit" class="btn tiny" disabled={!!pending || !source.trim()}>Accept source…</button>
          </form>
        </section>
      {:else}
        <p class="muted small">
          {r.backend === "none" ? "No firewall tool was found (ufw, firewalld, nftables or iptables)." : "Nightgrid can change ufw and firewalld; this host's rules are shown as they are."}
        </p>
      {/if}
      {#if r.raw.trim() && r.backend !== "firewalld"}
        <details class="raw" open={r.backend === "nftables" || r.backend === "iptables"}>
          <summary class="label">{r.backend === "iptables" ? "IPTABLES-SAVE" : "NFT LIST RULESET"}</summary>
          <pre class="mono small">{r.raw}</pre>
        </details>
      {/if}
      <p class="muted small">Every change saves the current rules on the host and arms a rollback there first. Keep a change once a new connection works; otherwise the host puts the old rules back by itself, even if Nightgrid can't reach it.</p>
    </div>
  {/if}
</div>

<Modal open={!!firewall.proposal} title="CHANGE THE FIREWALL" width={540} onclose={() => (firewall.proposal = null)}>
  {#if firewall.proposal}
    {@const p = firewall.proposal}
    <p><b>{hostName}</b>: {p.check.summary}.</p>
    {#if p.check.risk}
      <p class="bad">{p.check.risk}</p>
      <label class="field">
        <span>Type <b>{hostName}</b> to confirm</span>
        <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
      </label>
    {/if}
    <label class="field">
      <span>Put the old rules back unless kept within</span>
      <select class="input" bind:value={secs}><option value={60}>1 minute</option><option value={120}>2 minutes</option><option value={300}>5 minutes</option></select>
    </label>
    {#if applyError}<p class="bad small" role="alert">{applyError}</p>{/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (firewall.proposal = null)}>Cancel</button>
    <button type="button" class="btn primary" data-guide="firewall-apply" disabled={firewall.busy || (!!firewall.proposal?.check.risk && typed.trim() !== hostName)} onclick={apply}>{firewall.busy ? "Applying…" : "Apply with rollback"}</button>
  {/snippet}
</Modal>

<style>
  .fw {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .row,
  .add {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .mono {
    font-family: var(--font-mono, monospace);
  }
  .state {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
    border: 1px solid var(--line);
    padding: 0 6px;
  }
  .state.on {
    color: var(--ok);
    border-color: var(--ok);
  }
  .pending {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 8%, transparent);
    padding: 8px 10px;
    font-size: var(--fs-xs);
  }
  .pending p {
    margin: 4px 0 0;
  }
  .count {
    color: var(--warn);
    font-variant-numeric: tabular-nums;
  }
  .acts {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .notice {
    color: var(--cyan);
    margin: 0;
  }
  .scroll {
    overflow: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  section {
    border: 1px solid var(--line);
    padding: 6px 10px;
  }
  h3 {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .head {
    margin-bottom: 6px;
  }
  .rule {
    display: grid;
    grid-template-columns: 70px 1fr minmax(80px, 160px) auto;
    gap: 10px;
    align-items: center;
    padding: 3px 0;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .act {
    letter-spacing: 0.08em;
  }
  .act.allow {
    color: var(--ok);
  }
  .act.limit {
    color: var(--cyan);
  }
  .act.deny,
  .act.reject {
    color: var(--danger);
  }
  .add {
    margin-top: 8px;
  }
  .add .input {
    width: auto;
    flex: 1 1 120px;
  }
  .add .port {
    flex: 0 1 130px;
  }
  .add select {
    flex: 0 0 auto;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin: 4px 0;
    font-size: var(--fs-xs);
  }
  .k {
    width: 64px;
    color: var(--text-1);
    letter-spacing: 0.08em;
  }
  .chip {
    border: 1px solid var(--line);
    padding: 0 0 0 6px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .chip button {
    background: none;
    border: 0;
    border-left: 1px solid var(--line);
    color: var(--text-1);
    cursor: pointer;
    padding: 0 6px;
  }
  .chip button:hover:not(:disabled) {
    color: var(--danger);
  }
  .tag {
    font-size: 10px;
    letter-spacing: 0.08em;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 0 4px;
  }
  .tag.ok {
    border-color: var(--ok);
    color: var(--ok);
  }
  .raw pre {
    max-height: 300px;
    overflow: auto;
    margin: 6px 0 0;
    color: var(--text-1);
  }
  .raw summary {
    cursor: pointer;
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--danger);
  }
  .muted {
    color: var(--text-1);
  }
</style>
