<script lang="ts">
  /**
   * INCOMING TRANSMISSION (D-202): a signed notice from Nightgrid's developer,
   * shown as a channel opening: static, a signal locking on, a short decode,
   * then the message. Effects are skipped under reduce effects.
   */
  import Markdown from "../Markdown.svelte";
  import { broadcasts, linkTarget } from "../../stores/broadcasts.svelte";
  import { library } from "../../stores/library.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { sound } from "../../stores/sound.svelte";
  import { scramble, effectsEnabled } from "../../scramble";
  import { hexId } from "../../hex";
  import type { SectionId } from "../../sections";

  const b = $derived(broadcasts.current);
  const total = $derived(broadcasts.queue.length);
  const replaying = $derived(broadcasts.replay !== null);
  const LEVEL = { info: "INFO", warn: "ACTION NEEDED", critical: "CRITICAL" } as const;

  let dialog: HTMLDialogElement | undefined = $state();
  /** The message decodes in after a short "locking on" phase. */
  let decoded = $state(false);
  let shownId: string | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!dialog) return;
    const id = b?.id ?? null;
    if (id && id !== shownId) {
      shownId = id;
      decoded = false;
      clearTimeout(timer);
      if (!dialog.open) dialog.showModal();
      sound.play("transmission");
      if (effectsEnabled()) timer = setTimeout(() => (decoded = true), 650);
      else decoded = true;
    }
    if (!id) {
      shownId = null;
      if (dialog.open) dialog.close();
    }
  });

  const link = $derived(b?.link ? linkTarget(b.link.to) : null);

  function view(v: string) {
    if (!b) return;
    broadcasts.snooze(b);
    ui.go(v as SectionId);
  }
  function article(id: string) {
    if (!b) return;
    broadcasts.snooze(b);
    library.open(id);
  }
</script>

<dialog
  bind:this={dialog}
  class="tx {b?.level ?? 'info'}"
  aria-label="Incoming transmission"
  data-guide="transmission"
  oncancel={(e) => {
    e.preventDefault();
    if (b) broadcasts.snooze(b);
  }}
>
  {#if b}
    <div class="scan" aria-hidden="true"></div>
    <div class="static" class:on={!decoded} aria-hidden="true"></div>
    <header>
      <span class="sig" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
      <span class="head" use:scramble={"INCOMING TRANSMISSION"}></span>
      <span class="ch" aria-hidden="true">CH {hexId(b.id)}</span>
    </header>
    <div class="meta">
      <span><em>FROM</em> NIGHTGRID // DEV</span>
      <span class="ver" title="Signed with Nightgrid's release key and verified on this computer"><em>SIG</em> ✓ VERIFIED</span>
      <span class="lvl"><em>PRI</em> {LEVEL[b.level]}</span>
      {#if total > 1}<span class="count">{1} / {total}</span>{/if}
    </div>
    {#if !decoded}
      <div class="lock" role="status">
        <span>LOCKING SIGNAL</span>
        <div class="bar"><i></i></div>
      </div>
    {:else}
      <div class="msg">
        <h2 aria-label={b.title}><span use:scramble={b.title}></span></h2>
        <div class="body">
          <Markdown src={b.body} steps onview={view} onarticle={article} />
        </div>
      </div>
    {/if}
    <footer>
      <span class="end" aria-hidden="true">{decoded ? "— END OF MESSAGE —" : ""}</span>
      {#if b.link && link}
        <button type="button" class="btn tiny" disabled={!decoded} onclick={() => broadcasts.follow(b)}>{b.link.label}{link.kind === "web" ? " ↗" : " →"}</button>
      {/if}
      <button type="button" class="btn ghost tiny" onclick={() => broadcasts.snooze(b)}>{replaying ? "Close" : "Later"}</button>
      <button type="button" class="btn primary tiny" disabled={!decoded} onclick={() => broadcasts.acknowledge(b)}>Understood</button>
    </footer>
  {/if}
</dialog>

<style>
  .tx {
    --c: var(--cyan);
    position: relative;
    padding: 0;
    width: min(600px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    overflow: hidden;
    flex-direction: column;
    color: var(--text-0);
    background:
      repeating-linear-gradient(0deg, transparent 0 3px, color-mix(in srgb, var(--c) 4%, transparent) 3px 4px),
      radial-gradient(120% 80% at 0% 0%, color-mix(in srgb, var(--c) 12%, transparent), transparent 60%),
      var(--bg-1);
    border: 1px solid var(--c);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--c) 20%, transparent),
      0 0 36px color-mix(in srgb, var(--c) 28%, transparent);
    clip-path: polygon(0 0, calc(100% - 18px) 0, 100% 18px, 100% 100%, 18px 100%, 0 calc(100% - 18px));
  }
  .tx.warn {
    --c: var(--warn);
  }
  .tx.critical {
    --c: var(--danger);
  }
  .tx[open] {
    display: flex;
    animation: tune 420ms steps(6) both;
  }
  .tx::backdrop {
    background: color-mix(in srgb, var(--bg-0) 80%, transparent);
  }
  @keyframes tune {
    0% {
      opacity: 0;
      transform: scaleY(0.02);
      filter: brightness(3);
    }
    40% {
      opacity: 1;
      transform: scaleY(0.04) scaleX(1.02);
    }
    70% {
      transform: scaleY(1.03) skewX(-2deg);
      filter: brightness(1.6) hue-rotate(20deg);
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
    background: linear-gradient(180deg, transparent 0, color-mix(in srgb, var(--c) 14%, transparent) 50%, transparent 100%);
    height: 80px;
    animation: sweep 3.2s linear infinite;
    opacity: 0.5;
  }
  @keyframes sweep {
    from {
      transform: translateY(-80px);
    }
    to {
      transform: translateY(700px);
    }
  }
  .static {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: repeating-radial-gradient(circle at 37% 61%, color-mix(in srgb, var(--text-0) 12%, transparent) 0 1px, transparent 1px 3px);
    transition: opacity 200ms;
  }
  .static.on {
    opacity: 0.6;
    animation: jitter 90ms steps(2) infinite;
  }
  @keyframes jitter {
    50% {
      transform: translate(2px, -1px);
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
  .ver {
    color: var(--ok);
  }
  .count {
    margin-left: auto;
    color: var(--text-0);
  }
  .lock {
    display: grid;
    gap: 8px;
    padding: 34px 14px;
    justify-items: center;
    font-family: var(--font-display);
    letter-spacing: 0.2em;
    font-size: var(--fs-xs);
    color: var(--c);
  }
  .bar {
    width: 60%;
    height: 4px;
    background: color-mix(in srgb, var(--c) 15%, transparent);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    width: 100%;
    background: var(--c);
    transform-origin: left;
    animation: fill 600ms steps(12) forwards;
  }
  @keyframes fill {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }
  .msg {
    padding: 14px 14px 6px;
    overflow: auto;
    min-height: 0;
  }
  h2 {
    margin: 0 0 10px;
    font-family: var(--font-display);
    font-size: var(--fs-lg);
    letter-spacing: 0.04em;
    color: var(--text-0);
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
  :global(html[data-effects="off"]) .tx,
  :global(html[data-effects="off"]) .scan,
  :global(html[data-effects="off"]) .static,
  :global(html[data-effects="off"]) .sig i,
  :global(html[data-effects="off"]) .body {
    animation: none;
  }
  :global(html[data-effects="off"]) .scan {
    display: none;
  }
</style>
