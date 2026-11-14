<script lang="ts">
  import { scramble } from "../../scramble";
  import type { ConnectError } from "../../api";
  import { errorText } from "../../api";

  type SecretPrompt = Extract<ConnectError, { kind: "passwordRequired" } | { kind: "passphraseRequired" }>;
  interface Props {
    prompt: SecretPrompt;
    /** Remember is offered only for stored hosts with a working keychain. */
    canRemember: boolean;
    onsubmit: (secret: string, remember: boolean) => Promise<void>;
    oncancel: () => void;
  }
  let { prompt, canRemember, onsubmit, oncancel }: Props = $props();

  let secret = $state("");
  let remember = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  const isPass = $derived(prompt.kind === "passphraseRequired");

  function focus(node: HTMLInputElement) {
    node.focus();
  }

  async function submit(e: Event) {
    e.preventDefault();
    if (!secret) return;
    busy = true;
    error = null;
    try {
      const s = secret;
      secret = "";
      await onsubmit(s, remember);
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="prompt" onsubmit={submit} aria-labelledby="auth-title">
  <h2 id="auth-title">{isPass ? "Key passphrase" : "Password"} · {prompt.target}</h2>
  {#if prompt.retry}<p class="error-text" role="alert">That didn't work. Try again.</p>{/if}
  {#if prompt.kind === "passphraseRequired"}
    <p class="muted">Unlock <code>{prompt.path}</code></p>
  {/if}
  <input
    class="input"
    type="password"
    autocomplete="off"
    aria-label={isPass ? "Passphrase" : "Password"}
    bind:value={secret}
    use:focus
  />
  <label class="remember" class:disabled={!canRemember}>
    <input type="checkbox" bind:checked={remember} disabled={!canRemember} />
    <span>Remember in OS keychain</span>
  </label>
  {#if !canRemember}
    <p class="muted small">Not available for this hop (not a saved host, or no keychain).</p>
  {/if}
  {#if error}<p class="error-text" role="alert" use:scramble={error}></p>{/if}
  <div class="actions">
    <button type="button" class="btn ghost" onclick={oncancel}>Cancel</button>
    <button type="submit" class="btn primary" disabled={!secret || busy}>Connect</button>
  </div>
</form>

<style>
  .prompt {
    width: min(420px, 100%);
    padding: 18px 20px;
    border: 1px solid color-mix(in srgb, var(--mint) 45%, var(--line));
    background: var(--bg-1);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--fs-lg);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    word-break: break-all;
  }
  p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-xs);
  }
  code {
    color: var(--mint);
  }
  .remember {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .remember.disabled {
    opacity: 0.5;
    cursor: default;
  }
  input[type="checkbox"] {
    accent-color: var(--magenta);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
