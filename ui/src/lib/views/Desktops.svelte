<script lang="ts">
  /** Desktops (M10n): guest screens in tabs, and RDP tunnels. Opened from System → VMS. */
  import DesktopScreen from "../components/desktop/DesktopScreen.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import { desktopLabel, desktops } from "../stores/desktops.svelte";
  import { ui } from "../stores/ui.svelte";

  $effect(() => {
    void desktops.loadRdp();
  });
  const current = $derived(desktops.current);
</script>

<div class="desktops" data-guide="desktops">
  <div class="toolbar">
    <div class="tabs" role="tablist" aria-label="Open desktops">
      {#each desktops.tabs as t (t.key)}
        <span class="tab" class:on={desktops.active === t.key}>
          <button type="button" role="tab" aria-selected={desktops.active === t.key} onclick={() => (desktops.active = t.key)}>
            <span class="dot s-{t.status}" aria-hidden="true"></span>{desktopLabel(t)}
          </button>
          <button type="button" class="close" aria-label="Close {t.vm.name}" onclick={() => desktops.close(t.key)}>×</button>
        </span>
      {/each}
    </div>
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" onclick={() => ui.go("system")}>Open from System → VMS</button>
  </div>

  {#if desktops.rdp.length}
    <div class="rdp" data-guide="desktop-rdp">
      <span class="label">RDP</span>
      {#each desktops.rdp as r (r.id)}
        <span class="tun">
          <span>{r.guest}</span>
          <code>127.0.0.1:{r.localPort}</code>
          <span class="muted">→ {r.addr}</span>
          <button type="button" class="btn ghost tiny" onclick={() => desktops.stopRdp(r.id)}>Stop</button>
        </span>
      {/each}
    </div>
  {/if}

  <div class="pane">
    {#if current}
      {#key current.key}
        <DesktopScreen tab={current} />
      {/key}
    {:else}
      <div class="empty">
        <p class="label">NO DESKTOPS OPEN</p>
        <p class="muted">
          Open a virtual machine's screen from <strong>System → VMS</strong> (the <em>Screen</em> button on a running VM). It works for libvirt/KVM guests with a VNC display and Proxmox VMs, over the host's SSH
          connection: no port is opened and nothing needs installing. Windows guests can also use <em>RDP</em>, which opens your RDP client through an SSH tunnel.
        </p>
        <button type="button" class="btn primary" onclick={() => ui.go("system")}>Go to System</button>
      </div>
    {/if}
  </div>
</div>

<SudoPrompt />

<style>
  .desktops {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .spacer {
    flex: 1;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-width: 0;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--line);
  }
  .tab button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    padding: 3px 8px;
    cursor: pointer;
    letter-spacing: 0.04em;
  }
  .tab.on {
    border-color: var(--magenta);
  }
  .tab.on button:first-child {
    color: var(--magenta);
  }
  .tab .close {
    padding: 3px 6px;
    border-left: 1px solid var(--line);
  }
  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 6px;
    background: var(--text-1);
  }
  .dot.s-connected {
    background: var(--ok);
  }
  .dot.s-connecting {
    background: var(--cyan);
  }
  .rdp {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .label {
    letter-spacing: var(--tracking-label);
    color: var(--cyan);
  }
  .tun {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    border: 1px solid var(--line);
    padding: 2px 6px;
  }
  .pane {
    flex: 1;
    min-height: 0;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 20px;
    max-width: 70ch;
  }
</style>
