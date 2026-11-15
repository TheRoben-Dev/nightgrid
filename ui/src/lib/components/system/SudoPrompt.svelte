<script lang="ts">
  import Modal from "../Modal.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";

  const ask = $derived(system.sudoAsk);
  const host = $derived(system.host);
  const who = $derived(host ? `${host.username ? host.username + "@" : ""}${host.name}` : "this host");

  let password = $state("");
  let remember = $state(false);

  $effect(() => {
    if (ask) {
      password = "";
      remember = false;
    }
  });

  // After showModal() has moved focus to the dialog's first button.
  function focus(node: HTMLInputElement) {
    const t = setTimeout(() => node.focus(), 0);
    return { destroy: () => clearTimeout(t) };
  }

  function submit(e: Event) {
    e.preventDefault();
    if (!password) return;
    const p = password;
    password = "";
    system.answerSudo({ password: p, remember });
  }
</script>

<Modal open={!!ask} title="SUDO PASSWORD" width={440} onclose={() => system.answerSudo(null)}>
  {#if ask}
    <form id="sudo-form" class="body" onsubmit={submit}>
      <p>Nightgrid needs root on <strong>{who}</strong> to <strong>{ask.reason}</strong>.</p>
      {#if ask.retry}<p class="error-text" role="alert">That password didn't work. Try again.</p>{/if}
      <input class="input" type="password" autocomplete="off" aria-label="sudo password" bind:value={password} use:focus />
      <label class="remember" class:disabled={!ui.keychain}>
        <input type="checkbox" bind:checked={remember} disabled={!ui.keychain} />
        <span>Remember for this host in the OS keychain</span>
      </label>
      <p class="muted small">
        Sent to sudo on its standard input over SSH, never on a command line. Without "remember" it's forgotten when the
        action finishes.
      </p>
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => system.answerSudo(null)}>Cancel</button>
    <button type="submit" form="sudo-form" class="btn primary" disabled={!password}>Continue</button>
  {/snippet}
</Modal>

<style>
  .body {
    display: grid;
    gap: 10px;
  }
  p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .small {
    font-size: var(--fs-xs);
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
</style>
