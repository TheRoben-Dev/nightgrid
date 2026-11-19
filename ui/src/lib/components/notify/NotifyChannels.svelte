<script lang="ts">
  /**
   * Settings → ALERTS → NOTIFICATIONS (1.1, D-167): which notices reach the
   * desktop and a self-hosted ntfy server. The inbox (the top bar's INBOX)
   * always gets everything.
   */
  import { onMount } from "svelte";
  import { api, errorText, type NoticeLevel, type NotifySettings, type NtfyInfo } from "../../api";
  import { LEVELS, ntfyProblem, routeKey, SOURCES, toggleRoute } from "../../notices";
  import { settings } from "../../stores/settings.svelte";
  import Toggle from "../Toggle.svelte";

  const n = $derived(settings.value.notify);
  const set = (patch: Partial<NotifySettings>) => settings.update({ notify: { ...settings.value.notify, ...patch } });
  const setAlertsNative = (v: boolean) => settings.update({ alerts: { ...settings.value.alerts, native: v } });

  let info = $state<NtfyInfo | null>(null);
  let url = $state("");
  let token = $state("");
  let error = $state<string | null>(null);
  let result = $state<string | null>(null);
  let busy = $state(false);
  const urlProblem = $derived(url.trim() ? ntfyProblem(url) : null);

  onMount(() => {
    api.ntfyGet().then((i) => (info = i)).catch(() => {});
  });

  async function save(u: string | null, t: string | null) {
    error = null;
    try {
      info = await api.ntfySet(u, t);
      url = "";
      token = "";
    } catch (e) {
      error = errorText(e);
    }
  }

  async function test() {
    busy = true;
    result = null;
    error = null;
    try {
      const r = await api.notifyTest();
      const parts = ["Inbox: added"];
      if (r.native) parts.push(`Desktop: ${r.native}`);
      if (r.ntfy) parts.push(`ntfy: ${r.ntfy}`);
      result = parts.join(" · ");
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function on(channel: "native" | "ntfy", id: (typeof SOURCES)[number]["id"]): boolean {
    if (channel === "native" && id === "alert") return settings.value.alerts.native;
    return !n.muted.includes(routeKey(channel, id));
  }
  function flip(channel: "native" | "ntfy", id: (typeof SOURCES)[number]["id"], v: boolean) {
    if (channel === "native" && id === "alert") return setAlertsNative(v);
    return set({ muted: toggleRoute(n.muted, routeKey(channel, id), v) });
  }
</script>

<div class="routes" data-guide="settings-notify-routes">
  <p class="muted small">Everything lands in the <strong>INBOX</strong> in the top bar. Pick what also reaches the desktop and your own ntfy server.</p>
  <div class="grid" role="table" aria-label="Where notices go">
    <div class="r head" role="row"><span role="columnheader">NOTICE</span><span role="columnheader">DESKTOP</span><span role="columnheader">NTFY</span></div>
    {#each SOURCES as s (s.id)}
      <div class="r" role="row">
        <span role="cell" title={s.hint}>{s.label}</span>
        <span role="cell"><input type="checkbox" aria-label="{s.label} on the desktop" checked={on("native", s.id)} disabled={s.id !== "alert" && !n.native} onchange={(e) => flip("native", s.id, e.currentTarget.checked)} /></span>
        <span role="cell"><input type="checkbox" aria-label="{s.label} to ntfy" checked={on("ntfy", s.id)} disabled={!n.ntfy} onchange={(e) => flip("ntfy", s.id, e.currentTarget.checked)} /></span>
      </div>
    {/each}
  </div>
  <Toggle label="Desktop notifications for everything else" hint="Restore drills, rotations due, posture drops and capacity warnings. Alerts follow the switch above." checked={n.native} onchange={(v) => set({ native: v })} />
</div>

<div class="ntfy" data-guide="settings-ntfy">
  <Toggle
    label="ntfy (self-hosted)"
    hint="Push notices to your phone through your own ntfy server. Off by default; the public ntfy.sh isn't used."
    checked={n.ntfy}
    onchange={(v) => set({ ntfy: v })}
  />
  {#if n.ntfy}
    <p class="muted note">
      {#if info?.configured}Topic: <span class="data">{info.url}</span>{#if info.hasToken} · with an access token{/if}
        <button type="button" class="link" onclick={() => save(null, null)}>Remove</button>
      {:else}No server saved yet.{/if}
    </p>
    {#if info?.public}<p class="error small">This topic is on the public ntfy.sh (moved over from the old webhook). Replace it with your own server to keep notices off outside services.</p>{/if}
    <form
      class="row"
      onsubmit={(e) => {
        e.preventDefault();
        if (url.trim() && !urlProblem) void save(url, token.trim() ? token : null);
        else if (!url.trim() && token.trim() && info?.configured) void save("", token);
      }}
    >
      <input class="input url" placeholder="https://ntfy.home.lan/nightgrid" bind:value={url} autocomplete="off" spellcheck="false" aria-label="ntfy topic URL" />
      <input class="input tok" type="password" placeholder="access token (optional)" bind:value={token} autocomplete="off" aria-label="ntfy access token" />
      <button type="submit" class="btn ghost tiny" disabled={(!url.trim() && !token.trim()) || !!urlProblem}>{info?.configured ? "Replace" : "Save"}</button>
    </form>
    {#if urlProblem}<p class="error small">{urlProblem}</p>{/if}
    {#if info?.hasToken}<button type="button" class="link small" onclick={() => save("", "")}>Remove the access token</button>{/if}
    <div class="row">
      <span class="label">SEND</span>
      <div class="seg tabstrip sm" role="radiogroup" aria-label="Lowest level sent to ntfy">
        {#each LEVELS as l (l.id)}
          <button type="button" role="radio" aria-checked={n.ntfyLevel === l.id} class:on={n.ntfyLevel === l.id} onclick={() => set({ ntfyLevel: l.id as NoticeLevel })}>{l.label}</button>
        {/each}
      </div>
    </div>
    <Toggle label="Also send resolved and passed notices" checked={n.ntfyOk} onchange={(v) => set({ ntfyOk: v })} />
  {/if}
  <div class="row">
    <button type="button" class="btn ghost" disabled={busy} onclick={test}>{busy ? "Sending…" : "Send a test notice"}</button>
    {#if result}<span class="muted small" role="status">{result}</span>{/if}
  </div>
  {#if error}<p class="error small" role="alert">{error}</p>{/if}
</div>

<style>
  .routes,
  .ntfy {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 6px 0 10px;
  }
  .grid {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    max-width: 420px;
  }
  .r {
    display: grid;
    grid-template-columns: 1fr 80px 80px;
    align-items: center;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
  }
  .r:last-child {
    border-bottom: 0;
  }
  .r span:not(:first-child) {
    text-align: center;
  }
  .r.head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: var(--tracking-label);
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .url {
    flex: 1;
    min-width: 220px;
  }
  .tok {
    width: 190px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
    align-self: flex-start;
  }
  .note,
  .small {
    font-size: var(--fs-xs);
  }
  .error {
    color: var(--danger);
  }
  p {
    margin: 0;
  }
</style>
