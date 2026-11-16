<script lang="ts">
  /**
   * Fleet → ACCESS (M10u): who logged in to each host and from where, failed
   * attempts, bans, users with a shell, sudo rules and authorized keys.
   */
  import { access } from "../../stores/access.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { sshKeys } from "../../stores/sshkeys.svelte";
  import { ago, bannedNow, keyName, limits, reportText, shortFp, worstTone } from "../../access";
  import ExplainButton from "../helper/ExplainButton.svelte";

  let open = $state<number | null>(null);

  $effect(() => {
    if (!sshKeys.keys.length) void sshKeys.load();
  });

  const when = (ts: number) => new Date(ts * 1000).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  const SRC: Record<string, string> = { journal: "the journal", authLog: "auth.log", none: "nothing (needs root)" };
  const TOOL: Record<string, string> = { missing: "not installed", denied: "needs root", read: "" };

  function toggle(id: number) {
    open = open === id ? null : id;
    if (open === id && !access.reports[id] && !access.state[id]?.busy) void access.audit(id);
  }
</script>

<div class="access" data-guide="fleet-access">
  <div class="bar">
    <button type="button" class="btn primary tiny" data-guide="access-audit-all" disabled={access.auditingAll} onclick={() => void access.auditAll()}>
      {access.auditingAll ? "Auditing…" : "Audit all"}
    </button>
    <span class="muted small">Logins and failed attempts from the last 7 days, bans, users, sudo and keys. Read only; nothing is kept.</span>
  </div>

  <div class="scroll">
    <div class="row headrow" aria-hidden="true">
      <span>HOST</span><span class="n">LOGINS</span><span class="n">FAILED</span><span class="n">BANNED</span><span class="n">USERS</span><span class="n">KEYS</span><span>FINDINGS</span><span></span>
    </div>
    {#each hosts.hosts as h (h.id)}
      {@const r = access.reports[h.id]}
      {@const st = access.state[h.id]}
      <div class="item" class:open={open === h.id}>
        <button type="button" class="row head" aria-expanded={open === h.id} onclick={() => toggle(h.id)}>
          <span class="name"><i class="dot {r ? worstTone(r.findings) : 'muted'}"></i>{h.name}</span>
          {#if r}
            <span class="n">{r.logSource === "none" ? "—" : r.logins.length}</span>
            <span class="n" class:bad={r.failedTotal >= 50}>{r.logSource === "none" ? "—" : r.failedTotal}</span>
            <span class="n">{r.fail2ban === "read" || r.crowdsec === "read" ? bannedNow(r) : "—"}</span>
            <span class="n">{r.users.filter((u) => !u.system).length}</span>
            <span class="n">{r.keys.length}</span>
            <span class="find {worstTone(r.findings)}">{r.findings[0]?.text ?? "nothing stands out"}{r.findings.length > 1 ? ` (+${r.findings.length - 1})` : ""}</span>
          {:else}
            <span class="n muted">·</span><span class="n muted">·</span><span class="n muted">·</span><span class="n muted">·</span><span class="n muted">·</span>
            <span class="find muted">{st?.busy ? "reading…" : st?.error ? "" : "not audited"}</span>
          {/if}
          <span class="when muted small">{st?.busy ? "…" : r ? ago(r.at) : ""}</span>
        </button>
        {#if st?.error}<p class="bad small err" role="alert">{st.error}</p>{/if}
        {#if open === h.id && r}
          {@const lim = limits(r)}
          <div class="detail">
            <div class="acts">
              <span class="muted small">Read {r.asRoot ? "as root" : `as ${r.loginUser}`} · logins from {SRC[r.logSource]}</span>
              <span class="spacer"></span>
              <ExplainButton source="access audit" text={() => reportText(r, h.name)} question="Is anything in this access audit worrying, and what should I tighten first?" />
              <button type="button" class="btn ghost tiny" disabled={st?.busy} onclick={() => void access.audit(h.id)}>Audit again</button>
              {#if !r.asRoot}<button type="button" class="btn tiny" data-guide="access-root" disabled={st?.busy} onclick={() => void access.audit(h.id, true)}>Read as root…</button>{/if}
            </div>
            {#if lim.length}<p class="note small">Without root this can't see {lim.join(", ")}.</p>{/if}

            {#if r.findings.length}
              <ul class="findings">
                {#each r.findings as f, i (i)}<li class={f.level}><b>{f.level === "bad" ? "!!" : f.level === "warn" ? "!" : "i"}</b>{f.text}</li>{/each}
              </ul>
            {/if}

            <div class="grid">
              <section>
                <h4 class="label">LOGINS · {r.days} DAYS</h4>
                {#if r.logins.length}
                  <table>
                    <tbody>
                      {#each r.logins.slice(0, 50) as l, i (i)}
                        {@const kn = keyName(l.key, sshKeys.keys)}
                        <tr>
                          <td class="muted">{when(l.ts)}</td>
                          <td>{l.user}</td>
                          <td>{l.from}</td>
                          <td class={l.method === "publickey" ? "muted" : "warn"}>{l.method === "publickey" ? (kn ? `key ${kn}` : l.key ? `key ${shortFp(l.key)}` : "key") : l.method}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {:else}<p class="muted small">{r.logSource === "none" ? "Not readable without root." : "No SSH logins."}</p>{/if}
              </section>

              <section>
                <h4 class="label">FAILED · {r.failedTotal} FROM {r.failures.length} ADDRESS{r.failures.length === 1 ? "" : "ES"}</h4>
                {#if r.failures.length}
                  <table>
                    <tbody>
                      {#each r.failures.slice(0, 30) as f (f.from)}
                        <tr><td>{f.from}</td><td class="n bad">{f.count}</td><td class="muted">{f.users.join(", ")}</td><td class="muted">{ago(f.last)}</td></tr>
                      {/each}
                    </tbody>
                  </table>
                {:else}<p class="muted small">{r.logSource === "none" ? "Not readable without root." : "No failed attempts."}</p>{/if}
              </section>

              <section>
                <h4 class="label">BANS</h4>
                <p class="small">fail2ban: {TOOL[r.fail2ban] || `${r.jails.length} jail${r.jails.length === 1 ? "" : "s"}`} · CrowdSec: {TOOL[r.crowdsec] || `${r.decisions.length} decision${r.decisions.length === 1 ? "" : "s"}`}</p>
                {#each r.jails as j (j.name)}
                  <p class="small"><b>{j.name}</b> <span class="muted">{j.currentlyBanned} banned now · {j.totalBanned} in all · {j.currentlyFailed} failing</span></p>
                  {#if j.banned.length}<p class="mono small muted">{j.banned.slice(0, 20).join(" ")}{j.banned.length > 20 ? " …" : ""}</p>{/if}
                {/each}
                {#each r.decisions.slice(0, 20) as d, i (i)}
                  <p class="small"><span class="mono">{d.value}</span> <span class="muted">{d.kind} · {d.scenario} · {d.duration}</span></p>
                {/each}
              </section>

              <section>
                <h4 class="label">USERS WITH A SHELL</h4>
                <table>
                  <tbody>
                    {#each r.users as u (u.name)}
                      <tr class:muted={u.system}>
                        <td>{u.name}</td><td class="n muted">{u.uid}</td><td class="muted">{u.shell.split("/").pop()}</td>
                        <td>{#if u.sudo}<span class="tag">SUDO</span>{/if}{#if u.system}<span class="tag dim">SERVICE</span>{/if}</td>
                        <td class="n muted">{u.keys ? `${u.keys} key${u.keys === 1 ? "" : "s"}` : ""}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </section>

              <section class="wide">
                <h4 class="label">AUTHORIZED KEYS</h4>
                {#if r.keys.length}
                  <table>
                    <tbody>
                      {#each r.keys as k, i (i)}
                        {@const kn = keyName(k.key.fingerprint, sshKeys.keys)}
                        <tr>
                          <td>{k.user}</td>
                          <td class="muted">{k.key.algorithm.replace(/^ssh-/, "")}</td>
                          <td class="mono" title={k.key.fingerprint}>{shortFp(k.key.fingerprint)}</td>
                          <td>{k.key.comment}</td>
                          <td>{#if kn}<span class="tag ok">YOURS: {kn}</span>{/if}{#if k.key.options}<span class="tag dim" title={k.key.options}>OPTIONS</span>{/if}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                  <p class="muted small">Add, remove or rotate your own keys in Settings → Connections → SSH keys.</p>
                {:else}<p class="muted small">No keys readable.</p>{/if}
              </section>

              <section>
                <h4 class="label">SUDO RULES</h4>
                {#if r.sudoRules.length}
                  {#each r.sudoRules as s, i (i)}<p class="mono small" title={s.file}>{s.rule}</p>{/each}
                {:else}<p class="muted small">{r.sudoersReadable ? "None." : "Not readable without root."}</p>{/if}
              </section>

              <section>
                <h4 class="label">SSHD {r.sshdEffective ? "(EFFECTIVE)" : "(FROM CONFIG)"}</h4>
                {#if r.sshd.length}
                  {#each r.sshd as s (s.key)}<p class="mono small"><span class="muted">{s.key}</span> {s.value}</p>{/each}
                {:else}<p class="muted small">Defaults (nothing set or readable).</p>{/if}
              </section>

              {#if r.sessions.length}
                <section class="wide">
                  <h4 class="label">RECENT SESSIONS (last)</h4>
                  <pre class="mono small">{r.sessions.join("\n")}</pre>
                </section>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .access {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .bar,
  .acts {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
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
  .scroll {
    overflow: auto;
    min-height: 0;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(110px, 180px) 56px 60px 60px 50px 44px 1fr 70px;
    gap: 10px;
    align-items: center;
    padding: 4px 6px;
    font-size: var(--fs-xs);
  }
  .headrow {
    color: var(--text-1);
    letter-spacing: 0.08em;
    border-bottom: 1px solid var(--line);
  }
  .head {
    width: 100%;
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    font-size: var(--fs-xs);
    text-align: left;
    cursor: pointer;
  }
  .head:hover {
    background: var(--bg-1);
  }
  .item {
    border: 1px solid transparent;
  }
  .item.open {
    border-color: var(--line);
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    background: var(--line);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.bad {
    background: var(--danger);
  }
  .n {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .find {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .when {
    text-align: right;
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
  .err {
    margin: 0 6px 4px;
  }
  .detail {
    padding: 6px 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .note {
    margin: 0;
    color: var(--warn);
  }
  .findings {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: var(--fs-xs);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .findings b {
    display: inline-block;
    width: 22px;
  }
  .findings .info {
    color: var(--text-1);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 10px;
  }
  section {
    border: 1px solid var(--line);
    padding: 6px 8px;
    min-width: 0;
    max-height: 280px;
    overflow: auto;
  }
  section.wide {
    grid-column: 1 / -1;
  }
  section p {
    margin: 2px 0;
    overflow-wrap: anywhere;
  }
  h4 {
    margin: 0 0 4px;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    font-weight: 500;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-xs);
  }
  td {
    padding: 2px 6px 2px 0;
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
  }
  .tag {
    font-size: 10px;
    letter-spacing: 0.08em;
    border: 1px solid var(--magenta);
    color: var(--magenta);
    padding: 0 4px;
    margin-right: 4px;
  }
  .tag.dim {
    border-color: var(--line);
    color: var(--text-1);
  }
  .tag.ok {
    border-color: var(--ok);
    color: var(--ok);
  }
  pre {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    color: var(--text-1);
  }
</style>
