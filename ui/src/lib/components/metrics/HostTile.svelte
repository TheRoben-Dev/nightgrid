<script lang="ts">
  import type { Host, HostPing } from "../../api";
  import { formatSize } from "../../files/util";
  import { formatRate, formatUptime, metrics } from "../../stores/metrics.svelte";
  import { terms } from "../../stores/terms.svelte";
  import { scramble } from "../../scramble";
  import HostIcon from "../HostIcon.svelte";
  import Meter from "./Meter.svelte";
  import Sparkline from "./Sparkline.svelte";

  interface Props {
    host: Host;
    ping: HostPing | undefined;
  }
  let { host, ping }: Props = $props();

  const m = $derived(metrics.get(host.id));
  const s = $derived(m.sample);
  const memPct = $derived(s && s.memTotal ? (s.memUsed / s.memTotal) * 100 : null);
  const fullest = $derived(
    s?.mounts.reduce<{ mount: string; pct: number } | null>((best, x) => {
      const pct = x.used + x.avail ? (x.used / (x.used + x.avail)) * 100 : 0;
      return !best || pct > best.pct ? { mount: x.mount, pct } : best;
    }, null) ?? null,
  );
  const st = $derived(
    m.state === "ok" ? "linked" : m.state === "needsLogin" ? "login" : m.state === "offline" ? "offline" : m.state === "error" ? "error" : (ping?.state ?? "unknown"),
  );
  // The state word decodes in when it changes; live numbers stay plain.
  const stateWord = $derived(
    m.state === "ok"
      ? "UP"
      : m.state === "needsLogin"
        ? "LOGIN NEEDED"
        : m.state === "offline"
          ? "OFFLINE"
          : m.state === "error"
            ? "NO METRICS"
            : m.state === "off"
              ? "METRICS OFF"
              : "CONNECTING",
  );
  const stateExtra = $derived(
    m.state === "ok" && s ? formatUptime(s.uptimeSecs) : m.state === "offline" && m.retrySecs ? `· RETRY ${m.retrySecs}S` : "",
  );
  const pct = (v: number | null | undefined) => (v == null ? "--" : `${Math.round(v)}%`);
</script>

<div class="tile {st}">
  <span class="stripe" style:background={host.color ?? "var(--line)"} aria-hidden="true"></span>
  <button type="button" class="main" onclick={() => (metrics.detailId = host.id)} title="Open the monitor for {host.name}">
    <span class="head">
      <span class="id">
        <HostIcon art={host.icon} color={host.color} />
        <span class="hname">{host.name}{#if host.favorite}<span class="star" aria-hidden="true"> ★</span>{/if}</span>
        <span class="hhost">{host.username ? `${host.username}@` : ""}{host.hostname}</span>
      </span>
      {#if s?.cpuTemp != null}<span class="temp" class:hot={s.cpuTemp >= 80}>{Math.round(s.cpuTemp)}°C</span>{/if}
    </span>
    {#if s}
      <span class="rows">
        <span class="row">
          <span class="k">CPU</span><span class="v data">{pct(s.cpu)}</span>
          <span class="spark"><Sparkline values={m.cpu} max={100} height={18} label="CPU trend" /></span>
        </span>
        <span class="row">
          <span class="k">MEM</span><span class="v data" title="{formatSize(s.memUsed)} of {formatSize(s.memTotal)}">{pct(memPct)}</span>
          <span class="bar"><Meter value={memPct} label="Memory" /></span>
        </span>
        <span class="row">
          <span class="k">DSK</span><span class="v data" title={fullest?.mount}>{pct(fullest?.pct)}</span>
          <span class="bar"><Meter value={fullest?.pct ?? null} warn={80} crit={92} label="Fullest disk" /></span>
        </span>
        <span class="row net">
          <span class="k">NET</span>
          <span class="v data">↓{formatRate(s.netRx)} <span class="muted">↑{formatRate(s.netTx)}</span></span>
        </span>
      </span>
    {:else if m.state === "needsLogin"}
      <span class="note">Open a terminal on it once (password / host key), then metrics start.</span>
    {:else if m.message && m.state !== "connecting"}
      <span class="note" title={m.message}>{m.message}</span>
    {/if}
    <span class="hstate"><i aria-hidden="true"></i><span use:scramble={stateWord}></span>{#if stateExtra}<span>{stateExtra}</span>{/if}</span>
  </button>
  <span class="tools">
    {#if m.state === "offline" || m.state === "error"}
      <button type="button" class="act" title="Retry now" aria-label="Retry {host.name}" onclick={() => metrics.retry(host.id)}>↻</button>
    {/if}
    <button type="button" class="act" title="Shell" aria-label="Open a shell on {host.name}" onclick={() => terms.openHost(host.id)}>›_</button>
  </span>
</div>

<style>
  .tile {
    position: relative;
    border: 1px solid var(--line);
    background: var(--bg-0);
    transition:
      border-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out;
  }
  .tile:hover,
  .tile:focus-within {
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .stripe {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    padding: 10px 12px 10px 16px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    font: inherit;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 8px;
    padding-right: 52px;
  }
  .id {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .hname {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: var(--fs-lg);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }
  .star {
    color: var(--accent);
  }
  .hhost {
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .temp {
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    color: var(--cyan);
  }
  .temp.hot {
    color: var(--danger);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .row {
    display: grid;
    grid-template-columns: 30px 44px 1fr;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
  }
  .row.net {
    grid-template-columns: 30px 1fr;
  }
  .k {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .v {
    font-family: var(--font-mono);
    text-align: right;
  }
  .net .v {
    text-align: left;
  }
  .data {
    color: var(--cyan);
  }
  .muted {
    color: var(--text-1);
  }
  .note {
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .hstate {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--text-1);
  }
  .hstate i {
    width: 6px;
    height: 6px;
    background: var(--line);
  }
  .linked .hstate {
    color: var(--mint);
  }
  .linked .hstate i {
    background: var(--mint);
    box-shadow: 0 0 6px var(--mint);
  }
  .login .hstate {
    color: var(--accent);
  }
  .login .hstate i {
    background: var(--accent);
  }
  .offline .hstate,
  .error .hstate {
    color: var(--danger);
  }
  .offline .hstate i,
  .error .hstate i {
    background: var(--danger);
  }
  .tools {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 4px;
  }
  .act {
    min-width: 24px;
    height: 22px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    color: var(--text-1);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .act:hover {
    color: var(--magenta);
    border-color: var(--magenta);
  }
</style>
