<script lang="ts">
  /**
   * Quest updates (1.3): top right, under the top bar, styled like a quest
   * log entry. The label and title decode in (off under reduce effects, as is
   * the slide); click opens the guide on the quest's host.
   */
  import { questLog, type QuestToast } from "../../stores/questlog.svelte";
  import { scramble } from "../../scramble";

  const glyph = (t: QuestToast) => (t.kind === "complete" ? "◆" : t.kind === "objective" ? "◈" : t.kind === "started" ? "▶" : "◇");
</script>

<div class="toasts" aria-live="polite">
  {#each questLog.toasts as t (t.id)}
    <div class="toast {t.kind}" role="status">
      <button type="button" class="body" onclick={() => (questLog.dismiss(t.id), questLog.show(t.articleId, t.hostId))}>
        <span class="label"><span class="g" aria-hidden="true">{glyph(t)}</span> <span use:scramble={t.label}></span></span>
        <span class="quest"><span use:scramble={t.quest}></span>{#if t.host}<span class="host"> · {t.host}</span>{/if}</span>
        <span class="title">{t.title}</span>
        {#if t.next}<span class="next"><span class="nl">NEXT</span> {t.next}</span>{/if}
      </button>
      <button type="button" class="x" aria-label="Dismiss" onclick={() => questLog.dismiss(t.id)}>×</button>
      <span class="timer" aria-hidden="true"></span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    top: calc(var(--topbar-h) + 12px);
    right: 16px;
    z-index: 80;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(360px, calc(100vw - 32px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    position: relative;
    display: flex;
    background: color-mix(in srgb, var(--bg-1) 92%, var(--warn));
    border: 1px solid color-mix(in srgb, var(--warn) 60%, var(--line));
    border-left: 3px solid var(--warn);
    box-shadow:
      0 10px 30px rgb(0 0 0 / 50%),
      0 0 18px color-mix(in srgb, var(--warn) 18%, transparent);
    clip-path: polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 0 100%);
    animation: slide 220ms ease-out;
    overflow: hidden;
  }
  .toast.complete,
  .toast.objective {
    border-left-color: var(--mint);
  }
  .toast.complete .label,
  .toast.objective .label {
    color: var(--mint);
  }
  .body {
    all: unset;
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px 12px;
    cursor: pointer;
  }
  .body:focus-visible {
    outline: 1px solid var(--accent);
  }
  .label {
    font-family: var(--font-display);
    letter-spacing: 0.18em;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
  .quest {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    text-transform: uppercase;
  }
  .host {
    color: var(--text-2);
  }
  .title {
    color: var(--text-0);
    font-weight: 600;
  }
  .next {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .nl {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--warn);
    margin-right: 4px;
  }
  .x {
    all: unset;
    cursor: pointer;
    padding: 6px 10px;
    color: var(--text-2);
    align-self: flex-start;
  }
  .x:hover,
  .x:focus-visible {
    color: var(--text-0);
  }
  /* Time left before it goes. */
  .timer {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    width: 100%;
    background: var(--warn);
    transform-origin: left;
    animation: drain 7s linear forwards;
    opacity: 0.6;
  }
  @keyframes slide {
    from {
      transform: translateX(24px);
      opacity: 0;
    }
  }
  @keyframes drain {
    to {
      transform: scaleX(0);
    }
  }
  :global(:root[data-effects="off"]) .toast,
  :global(:root[data-effects="off"]) .timer {
    animation: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .toast,
    .timer {
      animation: none;
    }
  }
</style>
