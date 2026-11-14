<script lang="ts">
  import { scramble } from "../../scramble";
  import type { ConnectError } from "../../api";
  import { errorText } from "../../api";

  type KeyPrompt = Extract<ConnectError, { kind: "hostKeyUnknown" } | { kind: "hostKeyChanged" }>;
  interface Props {
    prompt: KeyPrompt;
    onaccept: () => Promise<void>;
    oncancel: () => void;
  }
  let { prompt, onaccept, oncancel }: Props = $props();

  let typed = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  const changed = $derived(prompt.kind === "hostKeyChanged");
  const previous = $derived(prompt.kind === "hostKeyChanged" ? prompt.previous : []);
  const confirmName = $derived(prompt.host);
  const canAccept = $derived(!changed || typed.trim() === confirmName);

  /** Per-character diff of a new fingerprint against an old one. */
  function diff(next: string, prev: string): { ch: string; same: boolean }[] {
    return [...next].map((ch, i) => ({ ch, same: prev[i] === ch }));
  }

  async function accept() {
    if (!canAccept) return;
    busy = true;
    error = null;
    try {
      await onaccept();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="prompt" class:changed role="alertdialog" aria-labelledby="hk-title" aria-describedby="hk-desc">
  {#if changed}
    <div class="banner" aria-hidden="true">
      {#each Array(3) as _, i (i)}<span>!! HOST KEY MISMATCH !!</span>{/each}
    </div>
    <h2 id="hk-title">Host key changed for {prompt.host}:{prompt.port}</h2>
    <p id="hk-desc">
      The server sent a different key than the one recorded before. This happens after a reinstall or key rotation, but it
      is also exactly what a man-in-the-middle attack looks like. The connection is blocked until you decide.
    </p>
    <div class="keys">
      {#each previous as p, i (i)}
        <div class="krow old">
          <span class="k">RECORDED</span>
          <span class="alg">{p.algorithm}</span>
          <code>{p.fingerprint}</code>
        </div>
      {/each}
      <div class="krow new">
        <span class="k">OFFERED</span>
        <span class="alg">{prompt.key.algorithm}</span>
        <code
          >{#each diff(prompt.key.fingerprint, previous[0]?.fingerprint ?? "") as d, i (i)}<span class:d={!d.same}
              >{d.ch}</span
            >{/each}</code
        >
      </div>
    </div>
    <label class="field confirm">
      <span>Type <strong>{confirmName}</strong> to replace the recorded key</span>
      <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" aria-invalid={typed !== "" && !canAccept} />
    </label>
  {:else}
    <h2 id="hk-title">First connection to {prompt.host}:{prompt.port}</h2>
    <p id="hk-desc">This host isn't known yet. Check the fingerprint against the server before trusting it:</p>
    <div class="keys">
      <div class="krow new">
        <span class="k">{prompt.key.algorithm}</span>
        <code>{prompt.key.fingerprint}</code>
      </div>
    </div>
    <p class="muted small">On the server: <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code></p>
  {/if}
  {#if error}<p class="error-text" role="alert" use:scramble={error}></p>{/if}
  <div class="actions">
    <button type="button" class="btn ghost" onclick={oncancel}>{changed ? "Disconnect" : "Cancel"}</button>
    <button type="button" class="btn {changed ? 'danger' : 'primary'}" disabled={!canAccept || busy} onclick={accept}>
      {changed ? "Accept new key" : "Trust & connect"}
    </button>
  </div>
</div>

<style>
  .prompt {
    max-width: 640px;
    width: 100%;
    padding: 18px 20px;
    border: 1px solid color-mix(in srgb, var(--mint) 45%, var(--line));
    background: var(--bg-1);
  }
  .prompt.changed {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 9%, var(--bg-1));
    box-shadow: 0 0 30px color-mix(in srgb, var(--danger) 25%, transparent);
  }
  .banner {
    display: flex;
    gap: 18px;
    overflow: hidden;
    white-space: nowrap;
    margin: -18px -20px 14px;
    padding: 4px 10px;
    background: var(--danger);
    color: var(--bg-0);
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.2em;
    font-size: var(--fs-sm);
  }
  h2 {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: var(--fs-lg);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .changed h2 {
    color: var(--danger);
  }
  p {
    margin: 0 0 12px;
    line-height: 1.5;
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .keys {
    border: 1px solid var(--line);
    margin-bottom: 12px;
  }
  .krow {
    display: grid;
    grid-template-columns: 80px auto 1fr;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    font-size: var(--fs-sm);
  }
  .krow + .krow {
    border-top: 1px solid var(--line);
  }
  .k {
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--text-1);
  }
  .alg {
    color: var(--text-1);
  }
  .old code {
    color: var(--text-1);
    text-decoration: line-through;
  }
  code {
    color: var(--mint);
    word-break: break-all;
  }
  .changed .new code {
    color: var(--text-0);
  }
  .d {
    color: var(--bg-0);
    background: var(--danger);
  }
  .confirm {
    margin-bottom: 12px;
  }
  .confirm strong {
    color: var(--text-0);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
