<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type AlertSettings, type WebhookInfo } from "../../api";
  import Panel from "../Panel.svelte";
  import SegmentSlider from "../SegmentSlider.svelte";
  import Toggle from "../Toggle.svelte";
  import NotifyChannels from "../notify/NotifyChannels.svelte";
  import { fromHhmm, toHhmm } from "../../stores/alerts.svelte";
  import { settings } from "../../stores/settings.svelte";

  const a = $derived(settings.value.alerts);
  const set = (patch: Partial<AlertSettings>) => settings.update({ alerts: { ...settings.value.alerts, ...patch } });

  const DISK_STEPS = [80, 85, 90, 95, 98] as const;
  const CERT_STEPS = [3, 7, 14, 21, 30] as const;
  const FORECAST_STEPS = [3, 7, 14, 30, 60] as const;
  const REPEAT_STEPS = [1, 2, 3, 5, 10] as const;
  const WINDOW_STEPS = [10, 20, 30, 60, 120] as const;
  const GRACE_STEPS = [0, 2, 5, 10, 15] as const;

  const mutes = $derived(a.logMutes ?? []);
  let muteSource = $state("");
  let muteRegex = $state("");
  let muteError = $state<string | null>(null);
  async function addMute() {
    muteError = null;
    const regex = muteRegex.trim();
    const source = muteSource.trim();
    if (!regex) return;
    try {
      new RegExp(regex);
    } catch (e) {
      muteError = `Not a valid regex: ${(e as Error).message}`;
      return;
    }
    const src = source && !source.includes("/") ? `container/${source}` : source;
    await set({ logMutes: [...mutes, { source: src, regex, fingerprint: "", label: regex }] });
    if (settings.error) muteError = settings.error;
    else muteRegex = "";
  }
  const removeMute = (i: number) => set({ logMutes: mutes.filter((_, j) => j !== i) });
  const WEBHOOKS = [
    { id: "none", label: "Off" },
    { id: "discord", label: "Discord" },
    { id: "json", label: "JSON POST" },
  ] as const;
  const PLACEHOLDER: Record<string, string> = {
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
    <Toggle label="Disk almost full" checked={a.diskFull} disabled={!a.enabled} onchange={(v) => set({ diskFull: v })} />
    {#if a.diskFull}
      <SegmentSlider label="DISK FULL AT" steps={DISK_STEPS} value={a.diskPercent} format={(v) => `${v}%`} onchange={(v) => set({ diskPercent: v })} />
      <Toggle
        label="Skip system mounts"
        hint="Read-only mounts and ones that are always full by design: WSL and Docker Desktop (/mnt/wsl/…), snaps, CD/DVD and disk images. To silence a real disk you know is full, use Ack or Mute on its alert."
        checked={a.diskSkipSystem}
        disabled={!a.enabled}
        onchange={(v) => set({ diskSkipSystem: v })}
      />
    {/if}
    <Toggle
      label="Disk full soon (forecast)"
      hint="From each mount's growth over the last two weeks, recorded while hosts are monitored. Shown on Fleet → DISKS."
      checked={a.diskForecast}
      disabled={!a.enabled}
      onchange={(v) => set({ diskForecast: v })}
    />
    {#if a.diskForecast}
      <SegmentSlider label="FULL WITHIN" steps={FORECAST_STEPS} value={a.forecastDays} format={(v) => `${v}d`} onchange={(v) => set({ forecastDays: v })} />
    {/if}
    <Toggle
      label="Storage pool degraded"
      hint="ZFS pools, RAID arrays and btrfs filesystems, read every 30 minutes on connected hosts (no root needed)."
      checked={a.poolDegraded}
      disabled={!a.enabled}
      onchange={(v) => set({ poolDegraded: v })}
    />
    <Toggle label="Service down" hint="Health check failing for about 90 seconds." checked={a.serviceDown} disabled={!a.enabled} onchange={(v) => set({ serviceDown: v })} />
    <Toggle label="Certificate or watched item expiring" hint="Service and proxy certificates within the days below; items on Services → Expiry watch within their own warning days." checked={a.certExpiring} disabled={!a.enabled} onchange={(v) => set({ certExpiring: v })} />
    {#if a.certExpiring}
      <SegmentSlider label="WARN WITHIN" steps={CERT_STEPS} value={a.certDays} format={(v) => `${v}d`} onchange={(v) => set({ certDays: v })} />
    {/if}
    <Toggle label="SMART warning" hint="Reads disk health every 6 hours on connected hosts with root, passwordless sudo or a remembered sudo password." checked={a.smartWarning} disabled={!a.enabled} onchange={(v) => set({ smartWarning: v })} />
    <Toggle label="Backup failed or stale" hint="Checks the jobs on System → Backups every 30 minutes on connected hosts." checked={a.backupStale} disabled={!a.enabled} onchange={(v) => set({ backupStale: v })} />
    <Toggle
      label="Log anomalies"
      hint="Every 10 minutes, reads new error lines from running containers and the journal on connected hosts, and alerts on bursts and errors never seen before. Off by default: it reads more over SSH."
      checked={a.logAnomaly}
      disabled={!a.enabled}
      onchange={(v) => set({ logAnomaly: v })}
    />
    {#if a.logAnomaly}
      <div class="logrules" data-guide="settings-log-rules">
        <p class="note">
          Only ERROR, FATAL and panic lines count (WARN, INFO and DEBUG never do), and a stack trace counts as one error. A new pattern alerts once it repeats; fewer become a note in the
          Alerts panel. FATAL/panic, or a container that's unhealthy or restarting, alerts at once, and so do exited or crash-looping containers.
        </p>
        <SegmentSlider label="ALERT AFTER" steps={REPEAT_STEPS} value={a.logRepeatCount ?? 3} format={(v) => `${v}×`} onchange={(v) => set({ logRepeatCount: v })} />
        <SegmentSlider label="WITHIN" steps={WINDOW_STEPS} value={a.logRepeatMinutes ?? 10} format={(v) => `${v} min`} onchange={(v) => set({ logRepeatMinutes: v })} />
        <SegmentSlider label="GRACE AFTER RESTART" steps={GRACE_STEPS} value={a.logGraceMinutes ?? 5} format={(v) => (v ? `${v} min` : "off")} onchange={(v) => set({ logGraceMinutes: v })} />
        <Toggle
          label="404s are noise"
          hint="HTTP 404 / NOT_FOUND lines never alert on their own (requests for missing pages, not faults). A spike well above the container's usual rate still does."
          checked={a.logNotFoundNoise ?? true}
          onchange={(v) => set({ logNotFoundNoise: v })}
        />
        <span class="label">MUTED PATTERNS</span>
        {#if mutes.length}
          <ul class="mutes" aria-label="Muted log patterns">
            {#each mutes as m, i (i)}
              <li>
                <span class="src">{m.source ? m.source.replace(/^container\//, "") : "any"}</span>
                <code title={m.fingerprint ? `fingerprint ${m.fingerprint}` : "regex"}>{m.label || m.regex || m.fingerprint}</code>
                <button type="button" class="btn ghost tiny" aria-label="Unmute {m.label}" onclick={() => removeMute(i)}>Remove</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="note">None yet. Use <b>Mute pattern</b> on a log alert or note, or add a regex here.</p>
        {/if}
        <div class="row">
          <input class="input" placeholder="container (any)" aria-label="Container to mute in" bind:value={muteSource} />
          <input class="input mono" placeholder="regex" aria-label="Regex to mute" bind:value={muteRegex} onkeydown={(e) => e.key === "Enter" && void addMute()} />
          <button type="button" class="btn tiny" disabled={!muteRegex.trim()} onclick={() => void addMute()}>Mute</button>
        </div>
        {#if muteError}<p class="error" role="alert">{muteError}</p>{/if}
        <p class="note">Muted lines are still counted: the Alerts panel shows how many were left out.</p>
      </div>
    {/if}
  </div>
</Panel>

<Panel title="NOTIFICATIONS" guide="settings-notifications">
  <div class="toggles">
    <Toggle label="Desktop notifications" hint="One per new alert; a burst becomes one summary." checked={a.native} onchange={(v) => set({ native: v })} />
    <Toggle label="Quiet hours" hint="Notices still land in the inbox, but nothing goes to the desktop, ntfy or webhooks." checked={a.quietHours} onchange={(v) => set({ quietHours: v })} />
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

  <NotifyChannels />

  <div class="row">
    <span class="label">WEBHOOK</span>
    <div class="seg tabstrip sm" role="radiogroup" aria-label="Webhook">
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
  .logrules {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-left: 12px;
    border-left: 1px solid var(--line);
  }
  .mutes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .mutes li {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .mutes .src {
    color: var(--text-1);
    min-width: 10ch;
  }
  .mutes code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--cyan);
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
</style>
