<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type AlertSettings, type WebhookInfo } from "../../api";
  import Panel from "../Panel.svelte";
  import SegmentSlider from "../SegmentSlider.svelte";
  import Toggle from "../Toggle.svelte";
  import { fromHhmm, toHhmm } from "../../stores/alerts.svelte";
  import { settings } from "../../stores/settings.svelte";

  const a = $derived(settings.value.alerts);
  const set = (patch: Partial<AlertSettings>) => settings.update({ alerts: { ...settings.value.alerts, ...patch } });

  const DISK_STEPS = [80, 85, 90, 95, 98] as const;
  const CERT_STEPS = [3, 7, 14, 21, 30] as const;
  const WEBHOOKS = [
    { id: "none", label: "Off" },
    { id: "ntfy", label: "ntfy" },
    { id: "discord", label: "Discord" },
    { id: "json", label: "JSON POST" },
  ] as const;
  const PLACEHOLDER: Record<string, string> = {
    ntfy: "https://ntfy.sh/your-topic",
    discord: "https://discord.com/api/webhooks/…",
    json: "https://hooks.example/nightgrid",
  };

  let hook = $state<WebhookInfo | null>(null);
  let url = $state("");
  let reveal = $state(false);
  let busy = $state(false);
  let result = $state<string | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    api.alertsWebhookGet().then((h) => (hook = h)).catch(() => {});
  });

  async function saveUrl(value: string | null) {
    error = null;
    try {
      hook = await api.alertsWebhookSet(value);
      url = "";
      reveal = false;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function test() {
    busy = true;
    result = null;
    error = null;
    try {
      const r = await api.alertsTest();
      const parts = [];
      if (r.native) parts.push(`Desktop: ${r.native}`);
      if (r.webhook) parts.push(`Webhook: ${r.webhook}`);
      result = parts.join(" · ") || "Nothing to test: notifications and webhook are off.";
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function time(field: "quietStart" | "quietEnd", v: string) {
    const m = fromHhmm(v);
    if (m !== null) void set({ [field]: m });
  }
</script>

<Panel title="ALERTS" guide="settings-alerts">
  <div class="toggles">
    <Toggle
      label="Alerts"
      hint="Watches what Nightgrid already monitors. Docker and SMART are only read on hosts that are already connected; SMART only where sudo needs no prompt."
      checked={a.enabled}
      onchange={(v) => set({ enabled: v })}
    />
  </div>
  <div class="rules" class:off={!a.enabled}>
    <Toggle label="Host offline" hint="Unreachable for 2 minutes (needs host metrics on)." checked={a.hostOffline} disabled={!a.enabled} onchange={(v) => set({ hostOffline: v })} />
    <Toggle label="Container stopped or unhealthy" hint="A container that was running exits with an error, restarts in a loop, or turns unhealthy." checked={a.containerDown} disabled={!a.enabled} onchange={(v) => set({ containerDown: v })} />
    <Toggle label="Disk filling up" checked={a.diskFull} disabled={!a.enabled} onchange={(v) => set({ diskFull: v })} />
    {#if a.diskFull}
      <SegmentSlider label="DISK FULL AT" steps={DISK_STEPS} value={a.diskPercent} format={(v) => `${v}%`} onchange={(v) => set({ diskPercent: v })} />
    {/if}
    <Toggle label="Service down" hint="Health check failing for about 90 seconds." checked={a.serviceDown} disabled={!a.enabled} onchange={(v) => set({ serviceDown: v })} />
    <Toggle label="Certificate expiring" checked={a.certExpiring} disabled={!a.enabled} onchange={(v) => set({ certExpiring: v })} />
    {#if a.certExpiring}
      <SegmentSlider label="WARN WITHIN" steps={CERT_STEPS} value={a.certDays} format={(v) => `${v}d`} onchange={(v) => set({ certDays: v })} />
    {/if}
    <Toggle label="SMART warning" hint="Reads disk health every 6 hours on connected hosts with root, passwordless sudo or a remembered sudo password." checked={a.smartWarning} disabled={!a.enabled} onchange={(v) => set({ smartWarning: v })} />
  </div>
</Panel>

<Panel title="NOTIFICATIONS" guide="settings-notifications">
  <div class="toggles">
    <Toggle label="Desktop notifications" hint="One per new alert; a burst becomes one summary." checked={a.native} onchange={(v) => set({ native: v })} />
    <Toggle label="Quiet hours" hint="Alerts still show in the app, but no notifications or webhooks." checked={a.quietHours} onchange={(v) => set({ quietHours: v })} />
  </div>
  {#if a.quietHours}
    <div class="row">
      <label class="fld"><span class="label">FROM</span>
        <input class="input hhmm" type="time" value={toHhmm(a.quietStart)} onchange={(e) => time("quietStart", e.currentTarget.value)} />
      </label>
      <label class="fld"><span class="label">UNTIL</span>
        <input class="input hhmm" type="time" value={toHhmm(a.quietEnd)} onchange={(e) => time("quietEnd", e.currentTarget.value)} />
      </label>
    </div>
  {/if}

  <div class="row">
    <span class="label">WEBHOOK</span>
    <div class="seg" role="radiogroup" aria-label="Webhook">
      {#each WEBHOOKS as w (w.id)}
        <button type="button" role="radio" aria-checked={a.webhook === w.id} class:on={a.webhook === w.id} onclick={() => set({ webhook: w.id })}>{w.label}</button>
      {/each}
    </div>
  </div>
  {#if a.webhook !== "none"}
    <p class="muted note">
      {#if hook?.configured}Saved in the keychain: <span class="data">{hook.host ?? "URL"}/…</span>
        <button type="button" class="link" onclick={() => saveUrl(null)}>Remove</button>
      {:else}No URL saved yet.{/if}
    </p>
    <form
      class="row"
      onsubmit={(e) => {
        e.preventDefault();
        if (url.trim()) void saveUrl(url);
      }}
    >
      <input
        class="input url"
        type={reveal ? "text" : "password"}
        placeholder={PLACEHOLDER[a.webhook]}
        bind:value={url}
        autocomplete="off"
        spellcheck="false"
        aria-label="Webhook URL"
      />
      <button type="button" class="btn ghost tiny" onclick={() => (reveal = !reveal)}>{reveal ? "Hide" : "Show"}</button>
      <button type="submit" class="btn ghost tiny" disabled={!url.trim()}>{hook?.configured ? "Replace" : "Save"}</button>
    </form>
    <div class="toggles">
      <Toggle label="Send resolved messages" checked={a.webhookResolved} onchange={(v) => set({ webhookResolved: v })} />
    </div>
  {/if}
  <div class="row">
    <button type="button" class="btn ghost" disabled={busy} onclick={test}>{busy ? "Sending…" : "Send test"}</button>
    {#if result}<span class="muted" role="status">{result}</span>{/if}
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</Panel>

<style>
  .toggles,
  .rules {
    display: flex;
    flex-direction: column;
  }
  .rules {
    padding-left: 12px;
    border-left: 1px solid var(--line);
    margin-top: 4px;
  }
  .rules.off {
    opacity: 0.55;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .fld {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .hhmm {
    width: 110px;
  }
  .url {
    flex: 1;
    min-width: 200px;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    padding: 3px 10px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .note {
    margin: 8px 0 0;
    font-size: var(--fs-xs);
  }
  .data {
    color: var(--cyan);
  }
  .link {
    border: 0;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .error {
    color: var(--danger);
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  .tiny {
    min-height: 22px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
