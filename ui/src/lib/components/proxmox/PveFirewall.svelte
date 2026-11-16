<script lang="ts">
  /**
   * The Proxmox firewall (M10r part 6) for one scope: options, ordered rules,
   * and at datacenter level its security groups and aliases. Host-level
   * changes run under a rollback the node arms by itself (Keep within 90 s).
   */
  import { onDestroy } from "svelte";
  import { errorText } from "../../api";
  import { blankRule, FW_ACTIONS, FW_LOGS, ruleProblem, ruleSpec, ruleText, scopeOf, type PveFwRuleSpec, type PveFwScope } from "../../proxmox";
  import { proxmox, scopeName } from "../../stores/proxmox.svelte";
  import type { PveFirewall } from "../../api";

  let { fixed = null }: { fixed?: PveFwScope | null } = $props();

  // The tab picks its scope; a guest's window passes a fixed one.
  let picked = $state<PveFwScope>(scopeOf("cluster"));
  const scope = $derived(fixed ?? picked);
  const key = $derived(JSON.stringify(scope));

  let fw = $state<PveFirewall | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  async function load() {
    const k = key;
    try {
      const f = await proxmox.loadFirewall(scope);
      if (k === key) {
        fw = f;
        error = null;
        syncOptions();
      }
    } catch (e) {
      if (k === key) error = errorText(e);
    }
  }
  let loadedFor = "";
  $effect(() => {
    if (proxmox.overview && key !== loadedFor) {
      loadedFor = key;
      fw = null;
      edit = null;
      void load();
    }
  });

  // Options.
  let enable = $state(false);
  let policyIn = $state("");
  let policyOut = $state("");
  function syncOptions() {
    enable = fw?.options?.enable ?? false;
    policyIn = fw?.options?.policyIn || (scope.level === "guest" ? "DROP" : scope.level === "cluster" ? "DROP" : "");
    policyOut = fw?.options?.policyOut || (scope.level === "node" ? "" : "ACCEPT");
  }
  const optsChanged = $derived(!!fw?.options && (enable !== fw.options.enable || (scope.level !== "node" && (policyIn !== (fw.options.policyIn || policyIn) || policyOut !== (fw.options.policyOut || policyOut)))));

  async function step(f: () => Promise<void>) {
    busy = true;
    try {
      await f();
    } catch {
      /* shown as a notice */
    } finally {
      busy = false;
      await load();
    }
  }
  function applyOptions() {
    const o = fw?.options;
    if (!o) return;
    const run = () => step(() => proxmox.fwOptions(scope, enable, scope.level === "node" ? "" : policyIn, scope.level === "node" ? "" : policyOut, o.digest));
    if (enable && !o.enable && scope.level !== "guest") {
      proxmox.confirm = {
        title: "TURN THE FIREWALL ON",
        body: `Turn on the ${scopeName(scope)} firewall? ${scope.level === "cluster" ? `Incoming traffic the rules don't allow is ${policyIn === "ACCEPT" ? "accepted" : "dropped"}. Proxmox always lets the cluster's local network reach SSH and the web UI, but if this computer reaches ${proxmox.localNode} some other way (a tailnet, VPN or another subnet), add an allow rule for it first.` : "Node rules only apply while the datacenter firewall is on."} A rollback is armed: unless you press Keep within 90 seconds, ${proxmox.localNode} restores its firewall by itself.`,
        button: `Turn on`,
        run: () => step(() => proxmox.fwOptions(scope, enable, scope.level === "node" ? "" : policyIn, scope.level === "node" ? "" : policyOut, o.digest)),
      };
      return;
    }
    void run();
  }

  // Rules.
  let edit = $state<{ pos: number | null; r: PveFwRuleSpec } | null>(null);
  const problem = $derived(edit ? ruleProblem(edit.r) : null);
  const addrs = $derived([...(fw?.aliases ?? []).map((a) => a.name), ...(fw?.ipsets ?? []).map((i) => `+${i.name}`)]);
  function saveRule() {
    if (!edit || problem || !fw) return;
    const e = edit;
    const digest = fw.digest;
    void step(async () => {
      await proxmox.saveRule(scope, e.pos, digest, e.r);
      edit = null;
    });
  }
  function toggle(pos: number) {
    const r = fw?.rules.find((x) => x.pos === pos);
    if (!r || !fw) return;
    const d = fw.digest;
    void step(() => proxmox.saveRule(scope, pos, d, { ...ruleSpec(r), enable: !r.enable }));
  }
  function move(pos: number, to: number) {
    if (!fw) return;
    const d = fw.digest;
    void step(() => proxmox.moveRule(scope, pos, to, d));
  }

  // Groups and aliases (datacenter).
  let newGroup = $state("");
  let aliasName = $state("");
  let aliasCidr = $state("");
  let aliasComment = $state("");

  // Session countdown.
  let now = $state(Date.now());
  const tick = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(tick));
  const left = $derived(proxmox.fwSession ? Math.max(0, Math.round((proxmox.fwSession.until - now) / 1000)) : 0);
  $effect(() => {
    if (proxmox.fwSession && left === 0) {
      proxmox.fwSession = null;
      void load();
    }
  });
  const nicKeys = $derived(scope.level === "guest" ? ["", "net0", "net1", "net2", "net3"] : []);
</script>

<div class="fw" data-guide="proxmox-firewall">
  {#if !fixed}
    <div class="bar">
      <span class="label">SCOPE</span>
      <select class="input" value={key} onchange={(e) => (picked = JSON.parse(e.currentTarget.value))} aria-label="Firewall scope">
        <option value={JSON.stringify(scopeOf("cluster"))}>Datacenter</option>
        {#if proxmox.localNode}<option value={JSON.stringify(scopeOf("node", { node: proxmox.localNode }))}>Node {proxmox.localNode}</option>{/if}
        {#each fw?.groups ?? [] as g (g.name)}<option value={JSON.stringify(scopeOf("group", { group: g.name }))}>Security group {g.name}</option>{/each}
        {#if picked.level === "group" && !(fw?.groups ?? []).some((g) => g.name === picked.group)}<option value={key}>Security group {picked.group}</option>{/if}
      </select>
      <span class="muted small">Guests' firewalls are in each guest's window.</span>
    </div>
  {/if}

  {#if proxmox.fwSession && scope.level !== "guest"}
    <div class="armed" role="alert">
      <strong>KEEP THE FIREWALL CHANGES?</strong>
      <span class="small">{proxmox.localNode} restores its firewall by itself in <span class="data">{left}</span> s unless you keep it. If this view still answers, the node is reachable.</span>
      <span class="sp"></span>
      <button type="button" class="btn tiny ghost" onclick={() => proxmox.settleFirewall(false).then(load)}>Roll back now</button>
      <button type="button" class="btn tiny primary" data-guide="proxmox-firewall-keep" onclick={() => proxmox.settleFirewall(true)}>Keep</button>
    </div>
  {:else if fw?.rollbackArmed && scope.level !== "guest"}
    <p class="warn small">A firewall rollback is armed on {proxmox.localNode}: it restores the previous rules shortly unless kept from the window that made the change.</p>
  {/if}

  {#if error}<p class="error-text small pre">{error}</p>{/if}
  {#if !fw}
    <p class="muted small">{error ? "" : "Reading…"}</p>
  {:else}
    {#if fw.notes.length}<p class="warn small pre">{fw.notes.join("\n")}</p>{/if}
    {#if fw.options}
      <div class="bar opts">
        <label class="check"><input type="checkbox" bind:checked={enable} /> Firewall on</label>
        {#if scope.level !== "node"}
          <label class="check">in <select class="input" bind:value={policyIn}>{#each FW_ACTIONS as a (a)}<option value={a}>{a}</option>{/each}</select></label>
          <label class="check">out <select class="input" bind:value={policyOut}>{#each FW_ACTIONS as a (a)}<option value={a}>{a}</option>{/each}</select></label>
        {/if}
        <span class="sp"></span>
        <button type="button" class="btn tiny primary" disabled={!optsChanged || busy} onclick={applyOptions}>Apply</button>
      </div>
      <p class="muted small">
        {#if scope.level === "cluster"}The datacenter switch turns the whole Proxmox firewall on or off; node and guest firewalls only work while it's on.
        {:else if scope.level === "node"}Rules for {scope.node} itself (SSH, web UI, cluster traffic). They apply while the datacenter firewall is on.
        {:else}Guest rules only filter network devices with "firewall" ticked (HARDWARE → Network), and only while the datacenter firewall is on.{/if}
      </p>
    {/if}

    <div class="bar">
      <strong class="small">RULES</strong><span class="muted small">top to bottom, first match wins</span>
      <span class="sp"></span>
      <button type="button" class="btn tiny" disabled={busy} onclick={() => (edit = { pos: null, r: blankRule() })}>+ Rule</button>
    </div>
    <ul class="rules">
      {#each fw.rules as r, i (r.pos)}
        <li class:off={!r.enable}>
          <span class="mono muted">{r.pos}</span>
          <input type="checkbox" checked={r.enable} disabled={busy} title={r.enable ? "Turn off" : "Turn on"} aria-label="Rule {r.pos} on" onchange={() => toggle(r.pos)} />
          <span class="dir {r.kind}">{r.kind.toUpperCase()}</span>
          <span class="act {r.action.toLowerCase()}">{r.action}</span>
          <span class="ell" title={ruleText(r)}>{r.kind === "group" ? "security group" : r.macro || [r.proto, r.dport].filter(Boolean).join(" ") || "any"}{#if r.source}<span class="muted">&nbsp;from&nbsp;</span><span class="mono">{r.source}</span>{/if}{#if r.dest}<span class="muted">&nbsp;to&nbsp;</span><span class="mono">{r.dest}</span>{/if}{#if r.iface}<span class="muted">&nbsp;on {r.iface}</span>{/if}</span>
          <span class="muted small ell cm" title={r.comment}>{r.comment}</span>
          <button type="button" class="btn tiny ghost" disabled={busy || i === 0} aria-label="Move rule {r.pos} up" onclick={() => move(r.pos, r.pos - 1)}>↑</button>
          <button type="button" class="btn tiny ghost" disabled={busy || i === fw.rules.length - 1} aria-label="Move rule {r.pos} down" onclick={() => move(r.pos, r.pos + 1)}>↓</button>
          <button type="button" class="btn tiny ghost" disabled={busy} onclick={() => (edit = { pos: r.pos, r: ruleSpec(r) })}>Edit…</button>
          <button type="button" class="btn tiny ghost" disabled={busy} onclick={() => proxmox.askDeleteRule(scope, r.pos, ruleText(r), fw?.digest ?? "", () => void load())}>Delete…</button>
        </li>
      {:else}
        <li class="muted small">No rules. {fw.options && !fw.options.enable ? "" : `With the firewall on, the ${scope.level === "node" ? "datacenter's" : "policy's"} defaults apply.`}</li>
      {/each}
    </ul>

    {#if edit}
      {@const r = edit.r}
      <form class="box" data-guide="proxmox-fw-rule" onsubmit={(e) => (e.preventDefault(), saveRule())}>
        <div class="line">
          <strong class="small">{edit.pos === null ? "NEW RULE (added on top)" : `RULE ${edit.pos}`}</strong>
          <select class="input" bind:value={r.kind} aria-label="Direction" onchange={() => (r.action = r.kind === "group" ? (fw?.groups[0]?.name ?? "") : "ACCEPT")}>
            <option value="in">in</option>
            <option value="out">out</option>
            {#if scope.level !== "group"}<option value="group">security group</option>{/if}
          </select>
          {#if r.kind === "group"}
            <select class="input" bind:value={r.action} aria-label="Group">{#each fw.groups as g (g.name)}<option value={g.name}>{g.name}</option>{/each}</select>
          {:else}
            <select class="input" bind:value={r.action} aria-label="Action">{#each FW_ACTIONS as a (a)}<option value={a}>{a}</option>{/each}</select>
            <select class="input" bind:value={r.macro} aria-label="Macro" title="A named service (ports and protocol filled in by Proxmox)">
              <option value="">no macro</option>
              {#each fw.macros as m (m.name)}<option value={m.name} title={m.comment}>{m.name}</option>{/each}
            </select>
          {/if}
          <label class="check"><input type="checkbox" bind:checked={r.enable} /> on</label>
        </div>
        {#if r.kind !== "group"}
          <div class="line">
            <label class="field"><span class="label">SOURCE</span><input class="input mono" list="pve-fw-addrs" bind:value={r.source} placeholder="any" spellcheck="false" /></label>
            <label class="field"><span class="label">DESTINATION</span><input class="input mono" list="pve-fw-addrs" bind:value={r.dest} placeholder="any" spellcheck="false" /></label>
            <datalist id="pve-fw-addrs">{#each addrs as a (a)}<option value={a}></option>{/each}</datalist>
            {#if !r.macro}
              <label class="field"><span class="label">PROTOCOL</span><input class="input mono narrow" list="pve-fw-protos" bind:value={r.proto} placeholder="any" spellcheck="false" /></label>
              <datalist id="pve-fw-protos"><option value="tcp"></option><option value="udp"></option><option value="icmp"></option><option value="ipv6-icmp"></option></datalist>
              <label class="field"><span class="label">DEST PORT</span><input class="input mono narrow" bind:value={r.dport} placeholder="any" spellcheck="false" /></label>
              <label class="field"><span class="label">SRC PORT</span><input class="input mono narrow" bind:value={r.sport} placeholder="any" spellcheck="false" /></label>
            {/if}
          </div>
        {/if}
        <div class="line">
          <label class="field"
            ><span class="label">INTERFACE</span>
            {#if nicKeys.length}<select class="input" bind:value={r.iface}>{#each nicKeys as k (k)}<option value={k}>{k || "any"}</option>{/each}</select>
            {:else}<input class="input mono narrow" bind:value={r.iface} placeholder="any" spellcheck="false" />{/if}
          </label>
          <label class="field"><span class="label">LOG</span><select class="input" bind:value={r.log}>{#each FW_LOGS as l (l)}<option value={l}>{l || "default"}</option>{/each}</select></label>
          <label class="field grow"><span class="label">COMMENT</span><input class="input" maxlength="200" bind:value={r.comment} /></label>
        </div>
        {#if problem}<p class="error-text small">{problem}</p>{/if}
        <div class="line">
          <span class="muted small">{ruleText(r)}</span>
          <span class="sp"></span>
          <button type="button" class="btn tiny ghost" onclick={() => (edit = null)}>Cancel</button>
          <button type="submit" class="btn tiny primary" disabled={!!problem || busy}>{edit.pos === null ? "Add" : "Save"}</button>
        </div>
      </form>
    {/if}

    {#if scope.level === "cluster"}
      <div class="bar"><strong class="small">SECURITY GROUPS</strong><span class="muted small">named rule sets you add to nodes and guests as one rule</span></div>
      <ul class="rules">
        {#each fw.groups as g (g.name)}
          <li>
            <span class="mono">{g.name}</span>
            <span class="muted small ell cm">{g.comment}</span>
            <button type="button" class="btn tiny ghost" onclick={() => (picked = scopeOf("group", { group: g.name }))}>Rules…</button>
            <button
              type="button"
              class="btn tiny ghost"
              disabled={busy}
              onclick={() => (proxmox.confirm = { title: "DELETE SECURITY GROUP", body: `Delete security group ${g.name}? Proxmox refuses while it still has rules or is used by a rule.`, button: `Delete ${g.name}`, run: () => step(() => proxmox.fwObject("group-delete", g.name)) })}>Delete…</button
            >
          </li>
        {/each}
      </ul>
      <form class="line pad" onsubmit={(e) => (e.preventDefault(), void step(() => proxmox.fwObject("group-add", newGroup)).then(() => (newGroup = "")))}>
        <input class="input mono" bind:value={newGroup} placeholder="new-group" aria-label="New group name" spellcheck="false" maxlength="18" />
        <button type="submit" class="btn tiny" disabled={busy || !/^[A-Za-z][A-Za-z0-9_-]{1,17}$/.test(newGroup)}>+ Group</button>
      </form>

      <div class="bar"><strong class="small">ALIASES</strong><span class="muted small">names for addresses, usable as source or destination</span></div>
      <ul class="rules">
        {#each fw.aliases as a (a.name)}
          <li>
            <span class="mono">{a.name}</span>
            <span class="mono small">{a.cidr}</span>
            <span class="muted small ell cm">{a.comment}</span>
            <button
              type="button"
              class="btn tiny ghost"
              disabled={busy}
              onclick={() => (proxmox.confirm = { title: "DELETE ALIAS", body: `Delete alias ${a.name} (${a.cidr})? Proxmox refuses while a rule uses it.`, button: `Delete ${a.name}`, run: () => step(() => proxmox.fwObject("alias-delete", a.name)) })}>Delete…</button
            >
          </li>
        {/each}
      </ul>
      <form class="line pad" onsubmit={(e) => (e.preventDefault(), void step(() => proxmox.fwObject("alias-add", aliasName, aliasCidr, aliasComment)).then(() => (aliasName = aliasCidr = aliasComment = "")))}>
        <input class="input mono" bind:value={aliasName} placeholder="name" aria-label="Alias name" spellcheck="false" />
        <input class="input mono" bind:value={aliasCidr} placeholder="192.168.1.0/24" aria-label="Address" spellcheck="false" />
        <input class="input grow" bind:value={aliasComment} placeholder="comment" aria-label="Comment" maxlength="200" />
        <button type="submit" class="btn tiny" disabled={busy || !/^[A-Za-z][A-Za-z0-9_-]{1,63}$/.test(aliasName) || !/^[0-9A-Fa-f.:/]+$/.test(aliasCidr)}>+ Alias</button>
      </form>
      {#if fw.ipsets.length}
        <p class="muted small pad">IP sets: {fw.ipsets.map((i) => `+${i.name}`).join(", ")}. Use them as source or destination; edit their entries in the web UI.</p>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .fw {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .line {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .bar {
    padding: 6px 2px 2px;
  }
  .line {
    align-items: end;
  }
  .pad {
    padding: 4px 2px;
    margin: 0;
  }
  .sp {
    margin-left: auto;
  }
  .armed {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    padding: 8px 10px;
    border: 1px solid var(--magenta);
  }
  .armed strong {
    color: var(--magenta);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .rules {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rules li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 2px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .rules li.off {
    opacity: 0.55;
  }
  .dir {
    font-size: var(--fs-xs);
    width: 44px;
    color: var(--text-1);
  }
  .act {
    font-size: var(--fs-xs);
    width: 92px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .act.accept {
    color: var(--ok);
  }
  .act.drop,
  .act.reject {
    color: var(--danger);
  }
  .ell {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cm {
    max-width: 30%;
  }
  .box {
    padding: 8px;
    border: 1px solid var(--cyan);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .grow {
    flex: 1;
    min-width: 160px;
  }
  .narrow {
    width: 110px;
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .warn {
    color: var(--warn);
    margin: 0;
  }
  p {
    margin: 0;
  }
  .pre {
    white-space: pre-wrap;
  }
</style>
