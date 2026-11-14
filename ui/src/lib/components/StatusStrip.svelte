<script lang="ts">
  import { onMount } from "svelte";
  import type { AppInfo } from "../api";
  import Barcode from "./Barcode.svelte";

  interface Props {
    info: AppInfo | null;
  }
  let { info }: Props = $props();

  let now = $state(new Date());
  onMount(() => {
    const t = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(t);
  });

  const pad = (n: number) => String(n).padStart(2, "0");
  const clock = $derived(`${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`);
  const keychainOk = $derived(info?.keychain.available ?? false);
</script>

<footer class="strip" aria-label="Status">
  <span class="cell"><span class="k">SESSIONS</span> 0</span>
  <span class="cell"><span class="k">XFER</span> 0</span>
  <span class="cell"><span class="k">ALERTS</span> 0</span>
  <span class="cell" class:ok={keychainOk} class:warn={!keychainOk} title={info?.keychain.detail ?? "OS keychain reachable"}>
    <span aria-hidden="true">{keychainOk ? "●" : "▲"}</span>
    <span class="k">KEYCHAIN</span>
    {keychainOk ? "OK" : "N/A"}
  </span>
  <span class="cell"><span class="k">DB</span> v{info?.schemaVersion ?? "-"}</span>
  <span class="spacer"><Barcode seed="strip" bars={56} /></span>
  <span class="cell micro">{info ? `${info.os.toUpperCase()}/${info.arch.toUpperCase()}` : "BOOT"}</span>
  <time class="cell clock">{clock}</time>
</footer>

<style>
  .strip {
    height: var(--strip-h);
    flex: none;
    display: flex;
    align-items: center;
    gap: 0;
    border-top: 1px solid var(--line);
    background: var(--bg-0);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
  }
  .cell {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    padding: 0 12px;
    height: 100%;
    border-right: 1px solid var(--line);
    color: var(--text-0);
  }
  .k {
    color: var(--text-1);
  }
  .ok > span:first-child {
    color: var(--ok);
  }
  .warn > span:first-child {
    color: var(--warn);
  }
  .spacer {
    flex: 1;
    display: flex;
    justify-content: center;
    min-width: 0;
    overflow: hidden;
  }
  .micro {
    border-left: 1px solid var(--line);
    color: var(--text-1);
  }
  .clock {
    border-right: 0;
    color: var(--cyan);
  }
</style>
