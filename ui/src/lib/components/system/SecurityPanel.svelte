<script lang="ts">
  /**
   * Security hygiene for the selected host (score, checks with suggested
   * fixes, listening ports) and a port map over every host. Read-only:
   * fixes are text to copy; Nightgrid never applies them.
   */
  import type { CheckStatus, ListenScope, Listener, SecurityCheck } from "../../api";
  import { writeClipboard } from "../../clipboard";
  import { inline } from "../../helper/md";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";

  let mode = $state<"host" | "all">("host");
  let copied = $state<string | null>(null);
  const r = $derived(system.security);
  const hostName = $derived(system.host?.name ?? "");

  const ORDER: Record<CheckStatus, number> = { fail: 0, warn: 1, unknown: 2, info: 3, pass: 4 };
  const checks = $derived([...(r?.checks ?? [])].sort((a, b) => ORDER[a.status] - ORDER[b.status]));
  const ICON: Record<CheckStatus, string> = { pass: "✓", warn: "!", fail: "✕", unknown: "?", info: "i" };
  const SCOPE: Record<ListenScope, string> = { local: "LOCAL", vpn: "VPN", address: "ONE ADDRESS", all: "ALL INTERFACES" };
  const SCOPE_HINT: Record<ListenScope, string> = {
    local: "Only this machine can connect.",
    vpn: "Reachable over a VPN address (Tailscale-style range).",
    address: "Bound to one address, usually the LAN.",
    all: "Bound to every interface: anything that can reach the host can try it (unless a firewall blocks it).",
  };

  const tone = (score: number) => (score >= 90 ? "ok" : score >= 60 ? "warn" : "bad");

  /** Code spans in a fix, for the copy buttons. */
  const commands = (fix: string) => inline(fix).filter((t) => t.kind === "code" && /\s/.test(t.text) && /^(sudo |[a-z-]+ )/.test(t.text)).map((t) => t.text);

  async function copy(text: string) {
    await writeClipboard(text);
    copied = text;
    setTimeout(() => copied === text && (copied = null), 1200);
  }

  const portLabel = (l: Listener) => `${l.port}/${l.proto}`;

  function explainText(): string {
    if (!r) return "";
    const lines = [`Security score ${r.score}/100 (${r.grade}).`, "Checks:"];
    for (const c of checks) lines.push(`- [${c.status}] ${c.title}: ${c.detail}`);
    lines.push("Listening ports:");
    for (const l of r.listeners)
      lines.push(`- ${portLabel(l)} ${l.service ?? ""} on ${l.addr} (${SCOPE[l.scope].toLowerCase()})${l.container ? `, container ${l.container}` : l.process ? `, ${l.process}` : ""}`);
    return lines.join("\n");
  }

  const mapRows = $derived(hosts.hosts.map((h) => ({ host: h, rep: system.portMap[h.id] })));
  const statusOf = (c: SecurityCheck) => c.status;
</script>

<div class="security">
  <div class="bar">
    <div class="seg" role="radiogroup" aria-label="Security view">
      <button type="button" role="radio" aria-checked={mode === "host"} class:on={mode === "host"} onclick={() => (mode = "host")}>THIS HOST</button>
      <button
        type="button"
        role="radio"
        aria-checked={mode === "all"}
        class:on={mode === "all"}
        data-guide="system-port-map"
        onclick={() => {
          mode = "all";
          if (!Object.keys(system.portMap).length || Object.keys(system.portMap).length < hosts.hosts.length) void system.scanAllPorts();
        }}>PORT MAP · ALL HOSTS</button
      >
    </div>
    <span class="spacer"></span>
    {#if mode === "host"}
      {#if r}<ExplainButton
          source="security audit"
          text={explainText}
          host={hostName || null}
          question="Go through this security audit. What should I fix first, and how, without locking myself out?"
        />{/if}
      <button type="button" class="btn ghost tiny" onclick={() => system.loadSecurity()} disabled={system.securityLoading}>{system.securityLoading ? "…" : "Re-check"}</button>
    {:else}
      <button type="button" class="btn ghost tiny" onclick={() => system.scanAllPorts()} disabled={system.portMapLoading}>{system.portMapLoading ? "Scanning…" : "Scan all hosts"}</button>
    {/if}
  </div>

  <div class="scroll">
    {#if mode === "host"}
      {#if system.errors.security}
        <p class="error-text pad">{system.errors.security}</p>
      {:else if !r}
        <p class="muted pad">{system.securityLoading ? "Checking…" : ""}</p>
      {:else}
        <div class="head">
          <div class="score {tone(r.score)}" aria-label="Score {r.score} of 100, grade {r.grade}">
            <span class="grade">{r.grade}</span>
            <span class="num">{r.score}<span class="muted">/100</span></span>
          </div>
          <div class="sum">
            <p>
              {checks.filter((c) => c.status === "fail").length} to fix · {checks.filter((c) => c.status === "warn").length} to review · {checks.filter(
                (c) => c.status === "pass",
              ).length} fine{#if checks.some((c) => c.status === "unknown")}
                · {checks.filter((c) => c.status === "unknown").length} couldn't be checked{/if}
            </p>
            <p class="muted small">Read without root, so nothing is changed and nothing prompts. Fixes are suggestions for you to run.</p>
          </div>
        </div>

        <ul class="checks" data-guide="system-security-checks">
          {#each checks as c (c.id)}
            <li class="check {statusOf(c)}">
              <span class="ic" aria-label={c.status}>{ICON[c.status]}</span>
              <div class="body">
                <p class="title">{c.title}{#if c.status === "fail" || c.status === "warn"}<span class="pts">−{c.status === "fail" ? c.weight : Math.floor(c.weight / 2)}</span>{/if}</p>
                <p class="detail">{c.detail}</p>
                {#if c.fix && c.status !== "pass"}
                  <p class="fix">
                    {#each inline(c.fix) as t, i (i)}{#if t.kind === "code"}<code>{t.text}</code>{:else if t.kind === "bold"}<b>{t.text}</b>{:else}{t.text}{/if}{/each}
                  </p>
                  {#each commands(c.fix) as cmd (cmd)}
                    <button type="button" class="btn ghost tiny copy" onclick={() => void copy(cmd)}>{copied === cmd ? "Copied" : `Copy: ${cmd}`}</button>
                  {/each}
                {/if}
              </div>
            </li>
          {/each}
        </ul>

        <h3 class="label">// LISTENING PORTS</h3>
        {#if !r.listeners.length}
          <p class="muted pad">No listening sockets reported.</p>
        {:else}
          <div class="ports" role="table" aria-label="Listening ports">
            <div class="prow phead" role="row">
              <span role="columnheader">PORT</span><span role="columnheader">SERVICE</span><span role="columnheader">REACHABLE FROM</span><span role="columnheader"
                >ADDRESS</span
              ><span role="columnheader">PROCESS</span>
            </div>
            {#each r.listeners as l (`${l.proto}${l.port}${l.addr}${l.container ?? ""}`)}
              <div class="prow" role="row">
                <code>{portLabel(l)}</code>
                <span>{l.service ?? ""}</span>
                <span class="scope {l.scope}" title={SCOPE_HINT[l.scope]}>{SCOPE[l.scope]}</span>
                <code class="muted">{l.addr}</code>
                <span class="muted">{l.container ? `container ${l.container}` : (l.process ?? "")}</span>
              </div>
            {/each}
          </div>
          {#if r.processesPartial}<p class="muted small pad">Process names show only for your own processes; the rest need root.</p>{/if}
        {/if}
      {/if}
    {:else}
      <div class="map" role="table" aria-label="Ports on every host">
        {#each mapRows as row (row.host.id)}
          <div class="mrow" role="row">
            <span class="mhost">
              <span class="hn">{row.host.name}</span>
              {#if row.rep && typeof row.rep !== "string"}<span class="mgrade {tone(row.rep.score)}" title="Security score {row.rep.score}/100">{row.rep.grade}</span>{/if}
            </span>
            <span class="chips">
              {#if row.rep === undefined}
                <span class="muted small">{system.portMapLoading ? "…" : "not scanned"}</span>
              {:else if typeof row.rep === "string"}
                <span class="muted small">{row.rep}</span>
              {:else}
                {#each row.rep.listeners.filter((l) => l.scope !== "local") as l (`${l.proto}${l.port}${l.addr}${l.container ?? ""}`)}
                  <span class="pchip {l.scope}" title="{l.addr} · {SCOPE[l.scope].toLowerCase()}{l.container ? ` · ${l.container}` : ''}"
                    >{l.port}{l.service ? ` ${l.service}` : ""}</span
                  >
                {:else}
                  <span class="muted small">nothing reachable from outside</span>
                {/each}
              {/if}
            </span>
          </div>
        {/each}
      </div>
      <p class="muted small pad legend">
        <span class="pchip all">all interfaces</span> <span class="pchip address">one address</span> <span class="pchip vpn">VPN</span> Localhost-only ports are left
        out. A firewall can still block what's listed.
      </p>
    {/if}
  </div>
</div>

<style>
  .security {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 9px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pad {
    padding: 8px 10px;
    margin: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .head {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 12px;
    border-bottom: 1px solid var(--line);
  }
  .score {
    --tone: var(--mint);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 84px;
    height: 84px;
    border: 1px solid var(--tone);
    box-shadow:
      0 0 12px color-mix(in srgb, var(--tone) 30%, transparent),
      inset 0 0 12px color-mix(in srgb, var(--tone) 15%, transparent);
  }
  .score.warn {
    --tone: var(--warn);
  }
  .score.bad {
    --tone: var(--danger);
  }
  .grade {
    font-family: var(--font-display);
    font-size: 30px;
    line-height: 1;
    color: var(--tone);
  }
  .num {
    font-variant-numeric: tabular-nums;
    font-size: var(--fs-sm);
  }
  .sum p {
    margin: 0 0 4px;
  }
  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .check {
    --tone: var(--text-2);
    display: flex;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    border-left: 2px solid transparent;
  }
  .check.pass {
    --tone: var(--mint);
  }
  .check.warn {
    --tone: var(--warn);
    border-left-color: var(--warn);
  }
  .check.fail {
    --tone: var(--danger);
    border-left-color: var(--danger);
  }
  .check.info {
    --tone: var(--cyan);
  }
  .ic {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border: 1px solid var(--tone);
    color: var(--tone);
    font-size: var(--fs-xs);
  }
  .body {
    min-width: 0;
  }
  .body p {
    margin: 0 0 3px;
  }
  .title {
    color: var(--text-0);
  }
  .pts {
    margin-left: 8px;
    color: var(--tone);
    font-size: var(--fs-xs);
  }
  .detail {
    color: var(--text-1);
    font-size: var(--fs-sm);
  }
  .fix {
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  code {
    font-family: var(--font-mono);
    color: var(--cyan);
  }
  .copy {
    text-transform: none;
    letter-spacing: 0;
    font-family: var(--font-mono);
    margin: 2px 6px 0 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h3 {
    margin: 0;
    padding: 10px 12px 4px;
    color: var(--cyan);
  }
  .prow {
    display: grid;
    grid-template-columns: 90px minmax(110px, 1fr) 130px minmax(110px, 1fr) minmax(120px, 1.2fr);
    gap: 10px;
    align-items: center;
    padding: 4px 12px;
    font-size: var(--fs-sm);
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .phead {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
  }
  .scope,
  .pchip {
    --tone: var(--text-2);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    color: var(--tone);
  }
  .scope.all,
  .pchip.all {
    --tone: var(--magenta);
  }
  .scope.address,
  .pchip.address {
    --tone: var(--accent);
  }
  .scope.vpn,
  .pchip.vpn {
    --tone: var(--mint);
  }
  .scope.local {
    --tone: var(--text-2);
  }
  .map {
    padding: 4px 0;
  }
  .mrow {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 12px;
    align-items: center;
    padding: 6px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .mhost {
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  .hn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mgrade {
    --tone: var(--mint);
    border: 1px solid var(--tone);
    color: var(--tone);
    font-family: var(--font-display);
    font-size: var(--fs-sm);
    line-height: 1.2;
    padding: 0 6px;
  }
  .mgrade.warn {
    --tone: var(--warn);
  }
  .mgrade.bad {
    --tone: var(--danger);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .pchip {
    border: 1px solid color-mix(in srgb, var(--tone) 60%, transparent);
    padding: 1px 6px;
    font-family: var(--font-mono);
  }
  .legend .pchip {
    margin-right: 4px;
  }
</style>
