<script lang="ts">
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import { SECTIONS } from "../sections";
  import { settings } from "../stores/settings.svelte";
  import { ui } from "../stores/ui.svelte";
  import Barcode from "./Barcode.svelte";
  import HostList from "./HostList.svelte";

  interface Props {
    version: string;
  }
  let { version }: Props = $props();
  const collapsed = $derived(settings.value.railCollapsed);
</script>

<nav class="rail" class:collapsed aria-label="Main">
  <div class="brand">
    <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
      <path d="M1.5 3.5H27.5L30.5 6.5V28.5H4.5L1.5 25.5Z" fill="none" stroke="var(--mint)" stroke-width="1.6" />
      <path d="M2 9H30" stroke="var(--mint)" stroke-width="1" opacity=".5" />
      <rect x="4.5" y="5.4" width="5.5" height="2" fill="var(--magenta)" />
      <rect x="11" y="5.4" width="5.5" height="2" fill="var(--mint)" />
      <path d="M7 14.5L11.5 18.5L7 22.5" fill="none" stroke="var(--cyan)" stroke-width="2.6" stroke-linecap="square" />
      <rect x="14.5" y="20.5" width="9" height="2.6" fill="var(--magenta)" />
    </svg>
    {#if !collapsed}
      <div class="wordmark">
        <span class="name">NIGHTGRID</span>
        <span class="micro">v{version} // HOMELAB.CC</span>
      </div>
    {/if}
  </div>

  {#if !collapsed}
    <HostList />
  {/if}

  <div class="group sections">
    {#if !collapsed}<div class="label group-label"><span class="cyan">//</span> MODULES</div>{/if}
    <ul>
      {#each SECTIONS as s (s.id)}
        <li>
          <button
            type="button"
            class="item"
            class:active={ui.section === s.id}
            aria-current={ui.section === s.id ? "page" : undefined}
            title={collapsed ? `${s.label} (${formatChord(DEFAULT_KEYMAP[s.action])})` : undefined}
            onclick={() => ui.go(s.id)}
          >
            <span class="code">{s.code}</span>
            {#if !collapsed}
              <span class="name">{s.label}</span>
              <kbd>{formatChord(DEFAULT_KEYMAP[s.action])}</kbd>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div class="foot">
    {#if !collapsed}<Barcode seed="rail" bars={36} />{/if}
    <button
      type="button"
      class="collapse"
      aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      title={formatChord(DEFAULT_KEYMAP["rail.toggle"])}
      onclick={() => settings.update({ railCollapsed: !collapsed })}
    >
      {collapsed ? "»" : "«"}
    </button>
  </div>
</nav>

<style>
  .rail {
    width: var(--rail-w);
    flex: none;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-1) 94%, transparent);
    transition: width var(--dur) var(--ease);
    min-height: 0;
  }
  .rail.collapsed {
    width: var(--rail-w-collapsed);
  }

  .brand {
    height: var(--topbar-h);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    color: var(--accent);
  }
  .mark {
    width: 26px;
    height: 26px;
    flex: none;
    filter: drop-shadow(var(--glow));
  }
  .wordmark {
    display: flex;
    flex-direction: column;
    line-height: 1.05;
  }
  .wordmark .name {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: var(--fs-lg);
    letter-spacing: 0.22em;
    color: var(--text-0);
  }
  .micro {
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--text-1);
  }

  .group {
    padding: 12px 8px 4px;
  }
  .group-label {
    padding: 0 6px 8px;
  }
  .cyan {
    color: var(--cyan);
  }
  .sections {
    flex: none;
    border-top: 1px solid var(--line);
  }
  .collapsed .sections {
    flex: 1;
    border-top: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item {
    width: 100%;
    min-height: 32px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    border: 1px solid transparent;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--text-1);
    position: relative;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .item:hover {
    background: var(--bg-2);
    color: var(--text-0);
  }
  .item.active {
    color: var(--text-0);
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 14%, transparent);
    box-shadow: var(--glow-active);
  }
  .item.active::before {
    content: "";
    position: absolute;
    left: -1px;
    top: -1px;
    bottom: -1px;
    width: 3px;
    background: var(--magenta);
  }
  .code {
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    width: 28px;
    color: inherit;
  }
  .item.active .code {
    color: var(--magenta);
  }
  .item .name {
    flex: 1;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-md);
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .collapsed .item {
    justify-content: center;
    padding: 0;
  }
  .collapsed .code {
    width: auto;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px;
    border-top: 1px solid var(--line);
  }
  .collapsed .foot {
    justify-content: center;
    padding: 8px 0;
  }
  .collapse {
    min-width: var(--hit);
    min-height: var(--hit);
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .collapse:hover {
    color: var(--text-0);
    border-color: var(--text-1);
  }
</style>
