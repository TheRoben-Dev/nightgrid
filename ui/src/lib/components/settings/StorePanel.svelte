<script lang="ts">
  // Settings → STORE (D-206): optional components. The core app keeps the
  // standard essentials; everything here is installed when wanted.
  import Panel from "../Panel.svelte";
  import { COMPONENTS, SHELVES, ownerOf, type StoreComponent } from "../../storeCatalog";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { componentPatches } from "../../stores/componentPatches.svelte";
  import { componentPacks } from "../../componentUi";

  const installedCount = $derived(COMPONENTS.filter((c) => settings.installed(c.id)).length);
  const asked = $derived(ui.featureAsked ? ownerOf(ui.featureAsked) : undefined);
  let busy = $state<string | null>(null);

  async function set(c: StoreComponent, on: boolean) {
    busy = c.id;
    try {
      await settings.setInstalled(c.id, on);
      // Fetch its newest patch right away (if one is published), or drop the stored one.
      if (on) void componentPatches.check(true, [c.id]);
      else await componentPatches.remove(c.id);
    } finally {
      busy = null;
    }
  }

  async function installAsked() {
    if (!asked) return;
    const view = ui.featureAsked;
    await set(asked, true);
    ui.featureAsked = null;
    if (view) ui.go(view as Parameters<typeof ui.go>[0]);
  }

  const day = (t: number) => new Date(t * 1000).toISOString().slice(0, 10);

  /** What to say about a component's patch, if it has one. */
  function patchLine(id: string): string | null {
    const p = componentPatches.pack(id);
    if (!p) return null;
    if (componentPacks.running.get(id) === p.built) return `Running patch from ${day(p.built)}.`;
    const why = componentPacks.skipped.get(id);
    if (why) return `Patch from ${day(p.built)} not used: ${why}.`;
    return `Patch from ${day(p.built)} ready; used the next time this view opens after a restart.`;
  }

  const CORE = "Hosts, Overview, Terminal, Files, Docker, Services, System, Fleet (updates, disks, backups, uptime), Tunnels, Network tools, Alerts, Knowledge and the Library.";
</script>

<div class="store" data-guide="settings-store">
  {#if asked && !settings.installed(asked.id)}
    <div class="asked" role="status">
      <span><b>{asked.label}</b> isn't installed.</span>
      <button type="button" class="btn primary tiny" disabled={busy !== null} onclick={installAsked}>Install and open</button>
      <button type="button" class="btn ghost tiny" onclick={() => (ui.featureAsked = null)}>Dismiss</button>
    </div>
  {/if}

  <Panel title="FEATURE STORE" prefix="//" count={COMPONENTS.length}>
    <p class="intro">
      Nightgrid's core is the everyday essentials: {CORE} Everything below is optional. Install what you use; a component that isn't installed is hidden, its code never loads and its background
      checks don't run. Removing one keeps its data, so installing it again brings everything back.
    </p>
    <p class="muted count">{installedCount} of {COMPONENTS.length} installed</p>
    <div class="patches">
      <button type="button" class="btn ghost tiny" disabled={componentPatches.checking} onclick={() => componentPatches.check(true)}>
        {componentPatches.checking ? "Checking…" : "Check for patches"}
      </button>
      <span class="muted">
        Components get fixes on their own between app releases, signed with the release key and checked before use.
        {#if componentPatches.checkedAt}Last checked {componentPatches.checkedAt.slice(11, 16)}.{/if}
      </span>
    </div>
    {#if componentPatches.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {componentPatches.error}</p>{/if}
  </Panel>

  {#each SHELVES as shelf (shelf.id)}
    {@const items = COMPONENTS.filter((c) => c.shelf === shelf.id)}
    {#if items.length}
      <Panel title={shelf.label} prefix=">" count={items.length}>
        <div class="grid">
          {#each items as c (c.id)}
            {@const on = settings.installed(c.id)}
            <article class="card" class:on class:hl={asked?.id === c.id}>
              <header>
                <span class="name">{c.label}</span>
                <span class="chip" class:ok={on}>{on ? "INSTALLED" : "AVAILABLE"}</span>
              </header>
              <p class="hint">{c.hint}</p>
              {#if on && patchLine(c.id)}
                {@const p = componentPatches.pack(c.id)}
                <p class="patch">{patchLine(c.id)}{#if p?.notes.length} {p.notes.slice(0, 3).join(" · ")}{/if}</p>
              {/if}
              <footer>
                {#if on}
                  <button type="button" class="btn ghost tiny" disabled={busy !== null} onclick={() => set(c, false)} aria-label="Remove {c.label}">Remove</button>
                {:else}
                  <button type="button" class="btn primary tiny" disabled={busy !== null} onclick={() => set(c, true)} aria-label="Install {c.label}">Install</button>
                {/if}
              </footer>
            </article>
          {/each}
        </div>
      </Panel>
    {/if}
  {/each}
  {#if settings.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {settings.error}</p>{/if}
</div>

<style>
  .store {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 1100px;
  }
  .asked {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 12px;
    border: 1px solid var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .asked span {
    flex: 1;
  }
  .intro {
    margin: 0;
    color: var(--text-1);
    max-width: 80ch;
  }
  .count {
    margin: 8px 0 0;
    font-size: var(--fs-xs);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-1) 70%, transparent);
  }
  .card.on {
    border-color: color-mix(in srgb, var(--mint) 45%, var(--line));
  }
  .card.hl {
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-0);
  }
  .chip {
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--text-1);
    border: 1px solid var(--line);
    padding: 1px 6px;
  }
  .chip.ok {
    color: var(--mint);
    border-color: color-mix(in srgb, var(--mint) 50%, var(--line));
  }
  .hint {
    margin: 0;
    flex: 1;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .patches {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: 10px;
    font-size: var(--fs-xs);
  }
  .patch {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
  .error {
    color: var(--danger);
  }
</style>
