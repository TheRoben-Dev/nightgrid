<script lang="ts">
  interface Props {
    label: string;
    hint?: string;
    checked: boolean;
    disabled?: boolean;
    onchange: (v: boolean) => void;
  }
  let { label, hint, checked, disabled = false, onchange }: Props = $props();
</script>

<label class="toggle" class:disabled>
  <input type="checkbox" {checked} {disabled} onchange={(e) => onchange(e.currentTarget.checked)} />
  <span class="box" aria-hidden="true"></span>
  <span class="text">
    <span class="name">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <span class="state" aria-hidden="true">{checked ? "ON" : "OFF"}</span>
</label>

<style>
  .toggle {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 12px;
    min-height: var(--hit);
    padding: 6px 0;
    cursor: pointer;
  }
  .disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .box {
    width: 16px;
    height: 16px;
    border: 1px solid var(--text-1);
    display: grid;
    place-items: center;
  }
  input:checked + .box {
    border-color: var(--mint);
    background: var(--mint);
    box-shadow: inset 0 0 0 3px var(--bg-1);
  }
  input:focus-visible + .box {
    outline: 1px solid var(--accent);
    outline-offset: 2px;
  }
  .text {
    display: flex;
    flex-direction: column;
  }
  .name {
    font-family: var(--font-display);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: var(--fs-sm);
  }
  .hint {
    color: var(--text-1);
    font-size: var(--fs-xs);
  }
  .state {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  input:checked ~ .state {
    color: var(--mint);
  }
</style>
