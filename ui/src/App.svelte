<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppInfo } from "./lib/api";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import LeftRail from "./lib/components/LeftRail.svelte";
  import StatusStrip from "./lib/components/StatusStrip.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import { DEFAULT_KEYMAP, matches, type ActionId } from "./lib/keys";
  import { buildCommands } from "./lib/palette/commands";
  import { scramble } from "./lib/scramble";
  import { SECTIONS, sectionById } from "./lib/sections";
  import { settings } from "./lib/stores/settings.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import Overview from "./lib/views/Overview.svelte";
  import Placeholder from "./lib/views/Placeholder.svelte";
  import Settings from "./lib/views/Settings.svelte";

  let info = $state<AppInfo | null>(null);
  let booted = $state(false);

  onMount(() => {
    const stop = settings.watchReducedMotion();
    settings.load().then(() => (booted = true));
    api
      .appInfo()
      .then((i) => (info = i))
      .catch((e) => console.error("app_info failed", e));
    return stop;
  });

  // Push theme/effects/scale onto <html> whenever they change.
  $effect(() => {
    settings.apply();
  });

  const commands = $derived(ui.paletteOpen ? buildCommands() : []);
  const section = $derived(sectionById(ui.section));

  const handlers: Partial<Record<ActionId, () => void>> = {
    "palette.open": () => (ui.paletteOpen = !ui.paletteOpen),
    "rail.toggle": () => settings.update({ railCollapsed: !settings.value.railCollapsed }),
    "effects.toggle": () => settings.update({ reduceEffects: !settings.value.reduceEffects }),
  };
  for (const s of SECTIONS) handlers[s.action] = () => ui.go(s.id);

  function onkeydown(e: KeyboardEvent) {
    for (const [action, spec] of Object.entries(DEFAULT_KEYMAP) as [ActionId, string][]) {
      if (matches(e, spec)) {
        // While the palette is open only its own toggle is global.
        if (ui.paletteOpen && action !== "palette.open") return;
        e.preventDefault();
        handlers[action]?.();
        return;
      }
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" aria-hidden="true"></div>
{#if settings.value.scanlines}<div class="scanlines" aria-hidden="true"></div>{/if}

<div class="shell" class:booted>
  <LeftRail version={info?.version ?? "0.1.0"} />
  <div class="main">
    <TopBar />
    <main class="content" aria-label={section.label}>
      <div class="heading">
        <h1 class="label">
          <span class="slash" aria-hidden="true">//</span>
          {#key section.id}<span use:scramble>{section.label.toUpperCase()}</span>{/key}
        </h1>
        <span class="micro" aria-hidden="true">SEC.{section.code} · {section.milestone}</span>
      </div>
      {#if section.id === "overview"}
        <Overview {info} />
      {:else if section.id === "settings"}
        <Settings {info} />
      {:else}
        <Placeholder {section} />
      {/if}
    </main>
    <StatusStrip {info} />
  </div>
</div>

<CommandPalette open={ui.paletteOpen} {commands} onclose={() => (ui.paletteOpen = false)} />

<style>
  .shell {
    position: relative;
    z-index: 1;
    height: 100%;
    display: flex;
    opacity: 0;
  }
  .shell.booted {
    opacity: 1;
    transition: opacity var(--dur-slow) var(--ease);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px 20px 24px;
  }
  .heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 14px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 700;
    color: var(--text-0);
    display: flex;
    gap: 10px;
  }
  .slash {
    color: var(--magenta);
  }
  .micro {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
</style>
