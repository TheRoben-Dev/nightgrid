<script lang="ts">
  /**
   * Services → EXPIRY WATCH (M10l): service and proxy certificates, plus
   * certificate files on hosts, domains, tokens and licences you add. Dates
   * of certificate files are read over SSH; a domain's date can be looked up
   * with `whois` on one of your hosts (only on a click). Rows found
   * automatically (service and proxy certificates) have their own Edit: trust
   * one self-signed certificate by its fingerprint, mute alerts, use your own
   * date or warning days.
   */
  import { onMount } from "svelte";
  import type { ExpiryPref, WatchInput, WatchKind } from "../../api";
  import { expiryCounts, expiryRows, fingerprintText, fromDateInput, leftText, sameTarget, toDateInput, type ExpiryRow } from "../../expiry";
  import { docker } from "../../stores/docker.svelte";
  import { expiry } from "../../stores/expiry.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { proxy } from "../../stores/proxy.svelte";
  import { services } from "../../stores/services.svelte";
  import { settings } from "../../stores/settings.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";
  import Panel from "../Panel.svelte";

  onMount(() => {
    if (!expiry.loaded) void expiry.load();
  });

  let now = $state(Math.floor(Date.now() / 1000));
  onMount(() => {
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 60_000);
    return () => clearInterval(t);
  });

  const hostName = (id: number | null | undefined) => (id == null ? "" : (hosts.byId(id)?.name ?? `host ${id}`));
  const rows = $derived(
    expiryRows(
      {
        items: expiry.items,
        services: services.services,
        status: services.status,
        proxy: proxy.report ? { report: proxy.report, hostName: hostName(docker.hostId) } : null,
        hostName,
        certDays: settings.value.alerts.certDays,
        prefs: expiry.prefs,
      },
      now,
    ),
  );
  const counts = $derived(expiryCounts(rows));

  // ---- editor ----
  const KINDS: { id: WatchKind; label: string; hint: string }[] = [
    { id: "certFile", label: "Certificate file on a host", hint: "A PEM file; Nightgrid reads its end date over SSH, twice a day while the host is connected." },
    { id: "domain", label: "Domain", hint: "Type the renewal date, or save and click Look up to ask whois on one of your hosts." },
    { id: "token", label: "API token or key", hint: "Only the name and date are kept, never the token itself." },
    { id: "license", label: "Licence or subscription", hint: "" },
    { id: "other", label: "Something else", hint: "" },
  ];
  const blank = (): WatchInput => ({ id: null, kind: "domain", name: "", hostId: null, target: "", expiresAt: null, warnDays: 30, note: "", muted: false });
  let form = $state<WatchInput>(blank());
  let date = $state("");
  let confirmDelete = $state(false);
  const editingItem = $derived(expiry.editing ? expiry.items.find((i) => i.id === expiry.editing) : undefined);
  $effect(() => {
    if (expiry.editing === null) return;
    const i = expiry.editing ? expiry.items.find((x) => x.id === expiry.editing) : undefined;
    form = i
      ? { id: i.id, kind: i.kind, name: i.name, hostId: i.hostId ?? null, target: i.target, expiresAt: i.expiresAt ?? null, warnDays: i.warnDays, note: i.note, muted: i.muted }
      : blank();
    date = toDateInput(i?.expiresAt ?? null);
    confirmDelete = false;
    expiry.error = null;
  });
  const kindHint = $derived(KINDS.find((k) => k.id === form.kind)?.hint ?? "");
  const valid = $derived(
    !!form.name.trim() &&
      form.warnDays >= 1 &&
      form.warnDays <= 365 &&
      (form.kind !== "certFile" || (form.hostId != null && form.target.trim().startsWith("/"))) &&
      (form.kind !== "domain" || /^[a-z0-9-]+(\.[a-z0-9-]+)+\.?$/i.test(form.target.trim())),
  );

  async function save(e: Event) {
    e.preventDefault();
    if (!valid) return;
    const input = { ...form, expiresAt: form.kind === "certFile" ? null : fromDateInput(date), warnDays: Number(form.warnDays) };
    if (await expiry.save(input)) expiry.editing = null;
  }

  // ---- edits for automatic rows (service and proxy certificates) ----
  const autoRow = $derived(expiry.editingAuto ? rows.find((r) => r.target && sameTarget(r.target, expiry.editingAuto!)) : undefined);
  const autoTls = $derived(autoRow?.target?.kind === "service" ? services.status[autoRow.target.serviceId]?.result.tls : undefined);
  const autoService = $derived(autoRow?.target?.kind === "service" ? services.services.find((x) => autoRow.target?.kind === "service" && x.id === autoRow.target.serviceId) : undefined);
  let auto = $state({ trust: false, muted: false, date: "", warn: "", note: "" });
  $effect(() => {
    if (!expiry.editingAuto) return;
    const r = autoRow;
    const p = r?.pref;
    auto = {
      // A changed certificate starts unticked: the new one has to be trusted on purpose.
      trust: r?.trust === "pinned",
      muted: !!p?.muted,
      date: toDateInput(p?.expiresAt ?? null),
      warn: p?.warnDays != null ? String(p.warnDays) : "",
      note: p?.note ?? "",
    };
    expiry.error = null;
  });
  const autoWarnOk = $derived(auto.warn.trim() === "" || (/^\d+$/.test(auto.warn.trim()) && Number(auto.warn) >= 1 && Number(auto.warn) <= 365));
  const canTrust = $derived(autoRow?.trust !== undefined && autoRow.trust !== "trusted" && !!autoRow.fingerprint);

  async function saveAuto(e?: Event) {
    e?.preventDefault();
    const r = autoRow;
    if (!r?.target || !autoWarnOk) return;
    const keepPin = r.trust === "pinned" || r.trust === "changed" ? (r.pref?.trustedFingerprint ?? null) : null;
    const pref: ExpiryPref = {
      target: r.target,
      trustedFingerprint: canTrust ? (auto.trust ? (r.fingerprint ?? null) : r.trust === "changed" ? keepPin : null) : null,
      muted: auto.muted,
      expiresAt: fromDateInput(auto.date),
      warnDays: auto.warn.trim() ? Number(auto.warn) : null,
      note: auto.note,
    };
    if (await expiry.savePref(pref)) expiry.editingAuto = null;
  }

  async function resetAuto() {
    const t = autoRow?.target;
    if (!t) return;
    if (await expiry.savePref({ target: t, trustedFingerprint: null, muted: false, expiresAt: null, warnDays: null, note: "" })) expiry.editingAuto = null;
  }

  function editRow(r: ExpiryRow) {
    if (r.item) expiry.editing = r.item.id;
    else if (r.target) expiry.editingAuto = r.target;
  }

  // ---- domain lookup ----
  let lookupVia = $state<Record<number, number>>({});
  const viaFor = (r: ExpiryRow) => lookupVia[r.item!.id] ?? r.item?.hostId ?? hosts.preferred("expiry")?.id;

  function explainText(): string {
    return rows
      .map((r) => `${r.kind} ${r.name} (${r.where}): ${r.expiresAt ? `${toDateInput(r.expiresAt)}, ${leftText(r.days)}` : "date unknown"}${r.error ? `; problem: ${r.error}` : ""}`)
      .join("\n");
  }
</script>

<Panel title="EXPIRY WATCH" prefix=">" count={rows.length} guide="services-expiry">
  {#snippet actions()}
    {#if counts.bad}<span class="gstat bad">{counts.bad} DUE</span>{/if}
    {#if counts.warn}<span class="gstat warn">{counts.warn} SOON</span>{/if}
  {/snippet}
  <div class="bar">
    <span class="muted small">Certificates, domains, tokens and licences, soonest first. Alerts use each item's warning days (certificates: Settings → Alerts).</span>
    <span class="spacer"></span>
    {#if rows.length}<ExplainButton source="expiry watch" text={explainText} question="Which of these need attention first, and what should I do to renew each one?" />{/if}
    {#if expiry.items.some((i) => i.kind === "certFile")}
      <button type="button" class="btn ghost tiny" disabled={expiry.busy} onclick={() => expiry.check()}>{expiry.busy ? "…" : "Read cert files"}</button>
    {/if}
    <button type="button" class="btn tiny" data-guide="expiry-add" onclick={() => (expiry.editing = 0)}>Add…</button>
  </div>
  {#if expiry.error && expiry.editing === null}<p class="error-text">{expiry.error}</p>{/if}
  {#if !rows.length}
    <p class="muted">Nothing to watch yet. HTTPS services show their certificate here once checked; add domains, tokens or certificate files with <strong>Add…</strong>.</p>
  {:else}
    <div class="table" role="table" aria-label="Expiry watch">
      {#each rows as r (r.key)}
        <div class="row {r.tone}" role="row">
          <span class="kind" role="cell">{r.kind}</span>
          <span class="name" role="cell">
            <span>{r.name}</span>
            <span class="muted small where">{r.where}{r.item?.note ? ` · ${r.item.note}` : r.pref?.note ? ` · ${r.pref.note}` : ""}</span>
            {#if r.error}<span class="err small">{r.error}</span>{/if}
            {#if r.trust === "pinned" || r.muted || r.pref?.expiresAt != null}
              <span class="muted small flags">
                {#if r.trust === "pinned"}<span title={r.problem ?? ""}>trusted by you ({fingerprintText(r.fingerprint).slice(0, 11)}…)</span>{/if}
                {#if r.muted}<span>alerts muted</span>{/if}
                {#if r.pref?.expiresAt != null}<span>your date{r.certExpiresAt ? `, certificate says ${toDateInput(r.certExpiresAt)}` : ""}</span>{/if}
              </span>
            {/if}
          </span>
          <span class="when" role="cell" title={r.expiresAt ? new Date(r.expiresAt * 1000).toLocaleString() : ""}>
            <span class="left">{leftText(r.days)}</span>
            <span class="muted small">{r.expiresAt ? toDateInput(r.expiresAt) : ""}</span>
          </span>
          <span class="acts" role="cell">
            {#if r.item?.kind === "domain" && hosts.hosts.length}
              <select
                class="input via"
                aria-label="Look up from host"
                value={viaFor(r)}
                onchange={(e) => (lookupVia = { ...lookupVia, [r.item!.id]: Number(e.currentTarget.value) })}
              >
                {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
              </select>
              <button
                type="button"
                class="btn ghost tiny"
                title="Run whois for {r.item.target} on that host (the host asks the registry)"
                disabled={expiry.looking.includes(r.item.id)}
                onclick={() => r.item && expiry.lookup(r.item.id, viaFor(r))}>{expiry.looking.includes(r.item.id) ? "…" : "Look up"}</button
              >
            {/if}
            {#if r.item}
              <button type="button" class="btn ghost tiny" onclick={() => editRow(r)}>Edit</button>
            {:else}
              <span class="muted small">{r.source === "service" ? "from Services" : "from Docker → Routes"}</span>
              <button type="button" class="btn ghost tiny" data-guide="expiry-edit" title="Trust, mute, or set your own date or warning days" onclick={() => editRow(r)}>Edit</button>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</Panel>

<Modal open={expiry.editing !== null} title={editingItem ? `EDIT :: ${editingItem.name}` : "WATCH AN EXPIRY DATE"} width={560} onclose={() => (expiry.editing = null)}>
  {#if confirmDelete && editingItem}
    <p>Stop watching <strong>{editingItem.name}</strong>? Nothing changes anywhere else.</p>
  {:else}
    <form class="grid" id="expiry-form" onsubmit={save}>
      <label class="field span6">
        <span>What</span>
        <select class="input" bind:value={form.kind}>
          {#each KINDS as k (k.id)}<option value={k.id}>{k.label}</option>{/each}
        </select>
        {#if kindHint}<span class="field-hint">{kindHint}</span>{/if}
      </label>
      <label class="field span6">
        <span>Name</span>
        <input class="input" bind:value={form.name} placeholder={form.kind === "token" ? "DNS API token" : form.kind === "domain" ? "Main domain" : "Name"} autocomplete="off" required />
      </label>
      {#if form.kind === "certFile" || form.kind === "domain"}
        <label class="field span3">
          <span>{form.kind === "certFile" ? "Host" : "Look up from (optional)"}</span>
          <select class="input" value={form.hostId ?? ""} onchange={(e) => (form.hostId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
            {#if form.kind !== "certFile"}<option value="">(choose when looking up)</option>{/if}
            {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
        </label>
        <label class="field span3">
          <span>{form.kind === "certFile" ? "Path" : "Domain"}</span>
          <input class="input" bind:value={form.target} placeholder={form.kind === "certFile" ? "/etc/ssl/certs/site.pem" : "example.com"} autocomplete="off" />
        </label>
      {:else}
        <label class="field span6">
          <span>Where (optional)</span>
          <input class="input" bind:value={form.target} placeholder="Provider, account or where it's used" autocomplete="off" />
        </label>
      {/if}
      {#if form.kind !== "certFile"}
        <label class="field span3">
          <span>Expires on</span>
          <input class="input" type="date" bind:value={date} />
        </label>
      {/if}
      <label class="field span3">
        <span>Warn days before</span>
        <input class="input" type="number" min="1" max="365" bind:value={form.warnDays} />
      </label>
      <label class="field span6">
        <span>Note</span>
        <input class="input" bind:value={form.note} placeholder="How to renew it (optional)" autocomplete="off" />
      </label>
      <label class="check span6">
        <input type="checkbox" bind:checked={form.muted} />
        <span>Mute alerts for this item <span class="muted small">(it stays on the list, greyed out, and never raises EXPIRING)</span></span>
      </label>
    </form>
  {/if}
  {#if expiry.error}<p class="error-text" role="alert">{expiry.error}</p>{/if}
  {#snippet footer()}
    {#if confirmDelete && editingItem}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button
        type="button"
        class="btn danger"
        onclick={async () => {
          const id = editingItem.id;
          expiry.editing = null;
          await expiry.remove(id);
        }}>Stop watching {editingItem.name}</button
      >
    {:else}
      {#if editingItem}<button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Remove…</button>{/if}
      <button type="button" class="btn ghost" onclick={() => (expiry.editing = null)}>Cancel</button>
      <button type="submit" form="expiry-form" class="btn primary" disabled={!valid || expiry.busy}>{editingItem ? "Save" : "Watch it"}</button>
    {/if}
  {/snippet}
</Modal>

<Modal open={expiry.editingAuto !== null} title={autoRow ? `EDIT :: ${autoRow.name}` : "EDIT"} width={580} onclose={() => (expiry.editingAuto = null)}>
  {#if autoRow}
    <form class="grid" id="expiry-auto-form" onsubmit={saveAuto}>
      <div class="span6 cert">
        <span class="muted small">{autoRow.source === "service" ? "Certificate of the service" : "Certificate of the proxy route"} · {autoRow.where}</span>
        {#if autoTls?.subject}<span class="small">Subject: {autoTls.subject}</span>{/if}
        {#if autoTls?.issuer}<span class="small">Issuer: {autoTls.issuer}</span>{/if}
        <span class="small">Expires (certificate): {autoRow.certExpiresAt ? toDateInput(autoRow.certExpiresAt) : "unknown"}</span>
        {#if autoRow.fingerprint}<span class="small mono">SHA-256: {fingerprintText(autoRow.fingerprint)}</span>{/if}
      </div>
      {#if canTrust}
        <label class="check span6">
          <input type="checkbox" bind:checked={auto.trust} />
          <span>
            Trust this certificate
            <span class="muted small"
              >It isn't trusted by this computer ({autoRow.problem}). Ticked, the warning goes away for this exact certificate only: if the server ever sends a different one, it warns again.</span
            >
            {#if autoRow.trust === "changed"}<span class="err small">The server now sends a different certificate than the one you trusted. Tick to trust the new one.</span>{/if}
          </span>
        </label>
        {#if autoService && !autoService.allowUntrusted}
          <p class="span6 muted small hint">
            This only changes Expiry Watch. The service's up/down check still counts it as down until you tick <strong>Allow untrusted certificate</strong> on the service itself.
          </p>
        {/if}
      {/if}
      <label class="check span6">
        <input type="checkbox" bind:checked={auto.muted} />
        <span>Mute expiry alerts <span class="muted small">(the row stays, greyed out; good for internal certificates you renew by hand or that don't run out)</span></span>
      </label>
      <label class="field span3">
        <span>Expires on (your date)</span>
        <input class="input" type="date" bind:value={auto.date} />
        <span class="field-hint">Empty: use the certificate's date.</span>
      </label>
      <label class="field span3">
        <span>Warn days before</span>
        <input class="input" inputmode="numeric" bind:value={auto.warn} placeholder={String(settings.value.alerts.certDays)} />
        <span class="field-hint">Empty: Settings → Alerts ({settings.value.alerts.certDays} days).</span>
      </label>
      <label class="field span6">
        <span>Note</span>
        <input class="input" bind:value={auto.note} placeholder="Why, or how it's renewed (optional)" autocomplete="off" />
      </label>
    </form>
  {:else}
    <p class="muted">This row isn't on the list any more.</p>
  {/if}
  {#if expiry.error}<p class="error-text" role="alert">{expiry.error}</p>{/if}
  {#snippet footer()}
    {#if autoRow?.pref}<button type="button" class="btn ghost left" disabled={expiry.busy} onclick={resetAuto}>Reset to found</button>{/if}
    <button type="button" class="btn ghost" onclick={() => (expiry.editingAuto = null)}>Cancel</button>
    <button type="submit" form="expiry-auto-form" class="btn primary" disabled={!autoRow || !autoWarnOk || expiry.busy}>Save</button>
  {/snippet}
</Modal>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .gstat {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    margin-left: 8px;
  }
  .gstat.bad {
    color: var(--danger);
  }
  .gstat.warn {
    color: var(--warn);
  }
  .table {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) 130px 270px;
    gap: 10px;
    align-items: center;
    padding: 4px 8px;
    border-left: 2px solid var(--line);
    font-size: var(--fs-sm);
  }
  .row.ok {
    border-left-color: var(--mint);
  }
  .row.warn {
    border-left-color: var(--warn);
  }
  .row.bad {
    border-left-color: var(--danger);
  }
  .row.muted {
    opacity: 0.6;
  }
  .flags {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .cert {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    border-left: 2px solid var(--line);
  }
  .mono {
    font-family: var(--font-mono, monospace);
    word-break: break-all;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }
  .check > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .hint {
    margin: 0;
  }
  .kind {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .err {
    color: var(--danger);
  }
  .when {
    display: flex;
    flex-direction: column;
  }
  .row.warn .left {
    color: var(--warn);
  }
  .row.bad .left {
    color: var(--danger);
  }
  .acts {
    display: flex;
    gap: 4px;
    align-items: center;
    justify-content: flex-end;
  }
  .via {
    width: 120px;
    padding: 1px 4px;
    font-size: var(--fs-xs);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 12px;
  }
  .span3 {
    grid-column: span 3;
  }
  .span6 {
    grid-column: span 6;
  }
  .error-text {
    margin: 8px 0 0;
  }
  :global(footer) .left {
    margin-right: auto;
  }
</style>
