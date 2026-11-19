<script lang="ts">
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import { RAIL, sectionById, type RailEntry, type SectionId } from "../sections";
  import { layout } from "../stores/layout.svelte";
  import { settings } from "../stores/settings.svelte";
  import { ui } from "../stores/ui.svelte";
  import Barcode from "./Barcode.svelte";
  import HostList from "./HostList.svelte";
  import Resizer from "./Resizer.svelte";

  interface Props {
    version: string;
  }
  let { version }: Props = $props();
  const collapsed = $derived(settings.value.railCollapsed);
  // Entries with at least one view turned on; an entry with one view left
  // takes that view's name.
  const entries = $derived(
    RAIL.map((r) => ({ ...r, on: r.members.filter((m) => settings.on(m)) }))
      .filter((r) => r.on.length)
      .map((r) => (r.on.length === 1 && r.members.length > 1 ? { ...r, label: sectionById(r.on[0]).label, code: sectionById(r.on[0]).code } : r)),
  );
  /** Where an entry goes: the view last open in it, else its first view that's on. */
  function target(r: RailEntry & { on: SectionId[] }): SectionId {
    const last = ui.lastInRail[r.id];
    return last && r.on.includes(last) ? last : r.on[0];
  }
  const keyFor = (r: RailEntry & { on: SectionId[] }) => formatChord(DEFAULT_KEYMAP[sectionById(target(r)).action]);
  const settingsOn = $derived(ui.section === "settings");
</script>

<nav
  class="rail"
  class:collapsed
  class:resizing={layout.dragging}
  style:width={collapsed ? undefined : `${layout.get("rail")}px`}
  aria-label="Main"
>
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
      {#each entries as r (r.id)}
        {@const active = r.members.includes(ui.section)}
        <li>
          <button
            type="button"
            class="item"
            data-guide="rail-{r.members[0]}"
            class:active
            aria-current={active ? "page" : undefined}
            title={collapsed ? `${r.label} (${keyFor(r)})` : r.on.length > 1 ? r.on.map((m) => sectionById(m).label).join(" · ") : undefined}
            onclick={() => ui.go(target(r))}
          >
            <span class="code">{r.code}</span>
            {#if !collapsed}
              <span class="name">{r.label}</span>
              {#if r.on.length > 1}<span class="more" aria-hidden="true">+{r.on.length - 1}</span>{/if}
              <kbd>{keyFor(r)}</kbd>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div class="foot">
    <button
      type="button"
      class="cfg"
      class:active={settingsOn}
      data-guide="rail-settings"
      aria-current={settingsOn ? "page" : undefined}
      title="Settings ({formatChord(DEFAULT_KEYMAP['nav.settings'])})"
      onclick={() => ui.go("settings")}
    >
      <span class="code">CFG</span>{#if !collapsed}<span class="name">Settings</span>{/if}
    </button>
    {#if !collapsed}<span class="bar"><Barcode seed="rail" bars={16} /></span>{/if}
    <button
      type="button"
      class="collapse"
      data-guide="rail-collapse"
      aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      title={formatChord(DEFAULT_KEYMAP["rail.toggle"])}
      onclick={() => settings.update({ railCollapsed: !collapsed })}
    >
      {collapsed ? "»" : "«"}
    </button>
  </div>
  {#if !collapsed}<Resizer key="rail" axis="x" overlay label="Resize sidebar" />{/if}
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
    position: relative;
    container: rail / inline-size;
  }
  .rail.resizing {
    transition: none;
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
    padding: 8px 8px 4px;
  }
  .group-label {
    padding: 0 6px 6px;
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
    gap: 1px;
  }
  .item {
    width: 100%;
    min-height: var(--hit);
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
    font-size: var(--fs-sm);
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .more {
    font-size: 10px;
    color: var(--text-1);
    opacity: 0.7;
  }
  /* Shortcuts show on hover and on the open entry, so the list stays calm. */
  .item kbd {
    position: absolute;
    right: 8px;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease);
  }
  .item:hover .more,
  .item:focus-visible .more,
  .item.active .more {
    opacity: 0;
  }
  .item:hover kbd,
  .item:focus-visible kbd,
  .item.active kbd {
    opacity: 1;
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
  .cfg {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    min-height: var(--hit);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    text-align: left;
  }
  .cfg:hover {
    background: var(--bg-2);
    color: var(--text-0);
  }
  .cfg.active {
    color: var(--text-0);
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 14%, transparent);
  }
  .cfg.active .code {
    color: var(--magenta);
  }
  .cfg .name {
    white-space: nowrap;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-sm);
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  /* The barcode is decoration: it shrinks first and leaves when the rail is narrow,
     so Settings and the collapse button always fit. */
  .bar {
    flex-grow: 0;
    flex-shrink: 100;
    flex-basis: auto;
    min-width: 0;
    overflow: hidden;
    display: flex;
  }
  @container rail (max-width: 216px) {
    .bar {
      display: none;
    }
  }
  .collapsed .cfg {
    flex: none;
    justify-content: center;
    padding: 0 4px;
  }
  .collapsed .foot {
    flex-direction: column;
  }
  .collapse {
    flex: none;
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
