<script lang="ts">
  /**
   * "What's new" (D-204): after an update, the release notes of every version
   * since the last one seen on this computer, from the CHANGELOG bundled with
   * the app (works offline). Styled as a patch download landing: a scan line,
   * the codename decoding in, then the notes. Effects are skipped under
   * reduce effects.
   */
  import Markdown from "../Markdown.svelte";
  import { releases } from "../../stores/releases.svelte";
  import { library } from "../../stores/library.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { sound } from "../../stores/sound.svelte";
  import { scramble } from "../../scramble";
  import { hexId } from "../../hex";
  import type { SectionId } from "../../sections";

  const list = $derived(releases.shown);
  let pick = $state(0);
  const r = $derived(list[Math.min(pick, list.length - 1)] ?? null);
  const newest = $derived(list[0] ?? null);
  const oldest = $derived(list[list.length - 1] ?? null);

  let dialog: HTMLDialogElement | undefined = $state();
  let body: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (list.length && !dialog.open) {
      pick = 0;
      dialog.showModal();
      sound.play("notice");
    }
    if (!list.length && dialog.open) dialog.close();
  });

  function choose(i: number) {
    pick = i;
    body?.scrollTo({ top: 0 });
  }
  function view(v: string) {
    releases.dismiss();
    ui.go(v as SectionId);
  }
  function article(id: string) {
    releases.dismiss();
    library.open(id);
  }
  function history() {
    releases.dismiss();
    ui.settingsTab = "general";
    ui.go("settings");
    requestAnimationFrame(() => document.querySelector('[data-guide="settings-releases"]')?.scrollIntoView({ block: "start", behavior: "smooth" }));
  }
</script>

<dialog
  bind:this={dialog}
  class="wn"
  aria-label="What's new"
  data-guide="whats-new"
  oncancel={(e) => {
    e.preventDefault();
    releases.dismiss();
  }}
>
  {#if r && newest}
    <div class="scan" aria-hidden="true"></div>
    <header>
      <span class="sig" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
      <span class="head" use:scramble={"SYSTEM UPGRADED"}></span>
      <span class="ch" aria-hidden="true">BUILD {hexId(newest.version)}</span>
    </header>
    <div class="meta">
      <span><em>NOW</em> v{newest.version}</span>
      {#if list.length > 1 && oldest}<span><em>SINCE</em> v{oldest.version}</span>{/if}
      <span><em>PATCHES</em> {list.length}</span>
      {#if newest.date}<span><em>DATE</em> {newest.date}</span>{/if}
    </div>
    {#if list.length > 1}
      <div class="tabs" role="tablist" aria-label="Releases">
        {#each list as x, i (x.version)}
          <button type="button" role="tab" aria-selected={i === pick} class:on={i === pick} onclick={() => choose(i)}>
            <span class="v">v{x.version}</span>
            {#if x.codename}<span class="cn">{x.codename}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    <div class="msg" bind:this={body}>
      {#key r.version}
        <p class="kicker">// CODENAME</p>
        <h2 aria-label={r.codename ?? `v${r.version}`}><span use:scramble={(r.codename ?? `v${r.version}`).toUpperCase()}></span></h2>
        <p class="sub">v{r.version}{r.date ? ` · ${r.date}` : ""}{r.tag ? ` · ${r.tag}` : ""}</p>
        <div class="body">
          {#if r.body}
            <Markdown src={r.body} onview={view} onarticle={article} />
          {:else}
            <p class="muted">No notes for this release.</p>
          {/if}
        </div>
      {/key}
    </div>
    <footer>
      <span class="end" aria-hidden="true">— END OF PATCH NOTES —</span>
      <button type="button" class="btn ghost tiny" onclick={history}>Release log →</button>
      <button type="button" class="btn primary tiny" onclick={() => releases.dismiss()}>Got it</button>
    </footer>
  {/if}
</dialog>

<style>
  .wn {
    --c: var(--mint);
    position: relative;
    padding: 0;
    width: min(680px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    overflow: hidden;
    flex-direction: column;
    color: var(--text-0);
    background:
      repeating-linear-gradient(0deg, transparent 0 3px, color-mix(in srgb, var(--c) 4%, transparent) 3px 4px),
      radial-gradient(120% 80% at 100% 0%, color-mix(in srgb, var(--magenta) 10%, transparent), transparent 55%),
      radial-gradient(120% 80% at 0% 0%, color-mix(in srgb, var(--c) 12%, transparent), transparent 60%),
      var(--bg-1);
    border: 1px solid var(--c);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--c) 20%, transparent),
      0 0 36px color-mix(in srgb, var(--c) 26%, transparent);
    clip-path: polygon(0 0, calc(100% - 18px) 0, 100% 18px, 100% 100%, 18px 100%, 0 calc(100% - 18px));
  }
  .wn[open] {
    display: flex;
    animation: boot 420ms steps(6) both;
  }
  .wn::backdrop {
    background: color-mix(in srgb, var(--bg-0) 80%, transparent);
  }
  @keyframes boot {
    0% {
      opacity: 0;
      transform: scaleY(0.02);
      filter: brightness(3);
    }
    50% {
      opacity: 1;
      transform: scaleY(1.03) skewX(-2deg);
      filter: brightness(1.5) hue-rotate(-20deg);
    }
    100% {
      transform: none;
      filter: none;
    }
  }
  .scan {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: linear-gradient(180deg, transparent 0, color-mix(in srgb, var(--c) 12%, transparent) 50%, transparent 100%);
    height: 80px;
    animation: sweep 3.6s linear infinite;
    opacity: 0.45;
  }
  @keyframes sweep {
    from {
      transform: translateY(-80px);
    }
    to {
      transform: translateY(800px);
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 26px 8px 14px;
    border-bottom: 1px solid color-mix(in srgb, var(--c) 45%, var(--line));
    background: linear-gradient(90deg, color-mix(in srgb, var(--c) 22%, transparent), transparent 75%);
  }
  .head {
    flex: 1;
    font-family: var(--font-display);
    letter-spacing: 0.22em;
    font-size: var(--fs-sm);
    color: var(--c);
    text-shadow: 0 0 8px color-mix(in srgb, var(--c) 60%, transparent);
  }
  .ch {
    font-size: var(--fs-xs);
    color: var(--text-1);
    font-family: var(--font-mono);
  }
  .sig {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 14px;
  }
  .sig i {
    width: 3px;
    background: var(--c);
    animation: bars 1.1s ease-in-out infinite;
  }
  .sig i:nth-child(1) {
    height: 30%;
  }
  .sig i:nth-child(2) {
    height: 50%;
    animation-delay: 0.1s;
  }
  .sig i:nth-child(3) {
    height: 70%;
    animation-delay: 0.2s;
  }
  .sig i:nth-child(4) {
    height: 85%;
    animation-delay: 0.3s;
  }
  .sig i:nth-child(5) {
    height: 100%;
    animation-delay: 0.4s;
  }
  @keyframes bars {
    50% {
      opacity: 0.25;
    }
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    padding: 6px 14px;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-1);
    border-bottom: 1px dashed color-mix(in srgb, var(--c) 30%, var(--line));
  }
  .meta em {
    font-style: normal;
    color: var(--c);
    margin-right: 4px;
  }
  .tabs {
    display: flex;
    gap: 6px;
    padding: 8px 14px 0;
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .tabs button {
    display: grid;
    gap: 1px;
    flex: none;
    padding: 4px 10px;
    text-align: left;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-1);
    background: transparent;
    border: 1px solid var(--line);
    cursor: pointer;
  }
  .tabs button.on {
    color: var(--text-0);
    border-color: var(--c);
    background: color-mix(in srgb, var(--c) 12%, transparent);
  }
  .tabs .v {
    color: var(--cyan);
  }
  .tabs .cn {
    letter-spacing: 0.06em;
  }
  .msg {
    padding: 12px 14px 6px;
    overflow: auto;
    min-height: 0;
  }
  .kicker {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    letter-spacing: 0.2em;
    color: var(--magenta);
  }
  h2 {
    margin: 2px 0 2px;
    font-family: var(--font-display);
    font-size: calc(var(--fs-lg) * 1.4);
    letter-spacing: 0.12em;
    color: var(--text-0);
    text-shadow:
      0 0 12px color-mix(in srgb, var(--c) 45%, transparent),
      2px 0 0 color-mix(in srgb, var(--magenta) 35%, transparent);
  }
  .sub {
    margin: 0 0 10px;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  .body {
    font-size: var(--fs-sm);
    line-height: 1.55;
    animation: reveal 380ms ease-out both;
  }
  @keyframes reveal {
    from {
      clip-path: inset(0 0 100% 0);
      opacity: 0.3;
    }
    to {
      clip-path: inset(0 0 0 0);
      opacity: 1;
    }
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px 12px;
    border-top: 1px solid color-mix(in srgb, var(--c) 30%, var(--line));
  }
  .end {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.12em;
  }
  :global(html[data-effects="off"]) .wn,
  :global(html[data-effects="off"]) .sig i,
  :global(html[data-effects="off"]) .body {
    animation: none;
  }
  :global(html[data-effects="off"]) .scan {
    display: none;
  }
</style>
