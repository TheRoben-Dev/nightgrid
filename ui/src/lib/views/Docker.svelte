<script lang="ts">
  import { scramble } from "../scramble";
  import { onMount } from "svelte";
  import type { ContainerView, DockerAction } from "../api";
  import ContainerPanel from "../components/docker/ContainerPanel.svelte";
  import RoutesPanel from "../components/docker/RoutesPanel.svelte";
  import StacksPanel from "../components/docker/StacksPanel.svelte";
  import Modal from "../components/Modal.svelte";
  import Resizer from "../components/Resizer.svelte";
  import AuthPrompt from "../components/terminal/AuthPrompt.svelte";
  import HostKeyPrompt from "../components/terminal/HostKeyPrompt.svelte";
  import { formatSize } from "../files/util";
  import { actionLabel, docker, isUp, type ContainerGroup } from "../stores/docker.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { layout } from "../stores/layout.svelte";
  import { terms } from "../stores/terms.svelte";
  import { ui } from "../stores/ui.svelte";
  import { targetLabel, targetUrl, webui } from "../stores/webui.svelte";

  onMount(() => docker.start());

  // Pick a host once the host list has loaded.
  $effect(() => {
    const first = docker.hostId === null ? hosts.preferred("docker") : undefined;
    if (first) docker.select(first.id);
  });

  let forceRemove = $state(false);
  let typed = $state("");
  $effect(() => {
    void docker.pending;
    forceRemove = false;
    typed = "";
  });

  const pending = $derived(docker.pending);
  const pendingAction = $derived<DockerAction | null>(
    pending ? (pending.action.kind === "remove" ? { kind: "remove", force: forceRemove } : pending.action) : null,
  );
  const runningTargets = $derived(pending ? pending.targets.filter(isUp) : []);
  /** Removing several containers needs the host name typed. */
  const needsTyping = $derived(!!pending && pending.action.kind === "remove" && pending.targets.length > 1);
  const hostName = $derived(docker.host?.name ?? "");
  const confirmOk = $derived(
    !!pending && (!needsTyping || typed.trim() === hostName) && !(pending.action.kind === "remove" && runningTargets.length && !forceRemove),
  );

  const consequence: Record<string, string> = {
    stop: "Each container gets SIGTERM, then SIGKILL after its grace period.",
    restart: "Each container is stopped and started again.",
    kill: "Each container gets SIGKILL immediately; it can't clean up.",
    remove: "The containers are deleted. Named volumes are kept; anything else inside the containers is lost.",
  };

  const cpu = (c: ContainerView) => {
    const s = docker.stats[c.id];
    return s ? `${(s.cpuPermille / 10).toFixed(1)}%` : "";
  };
  const mem = (c: ContainerView) => {
    const s = docker.stats[c.id];
    return s ? formatSize(s.memUsed) : "";
  };
  const memTitle = (c: ContainerView) => {
    const s = docker.stats[c.id];
    return s && s.memLimit ? `${formatSize(s.memUsed)} of ${formatSize(s.memLimit)}` : "";
  };
  const ports = (c: ContainerView) =>
    c.ports
      .filter((p) => p.public)
      .map((p) => `${p.public}→${p.private}${p.proto === "tcp" ? "" : "/" + p.proto}`)
      .join(" ");
  /** Docker's status minus the health suffix, which has its own column. */
  const uptime = (c: ContainerView) => c.status.replace(/\s*\((healthy|unhealthy|health: starting)\)/, "");

  const groupAll = (g: ContainerGroup) => g.containers.every((c) => docker.selected.includes(c.id));
  const groupSome = (g: ContainerGroup) => !groupAll(g) && g.containers.some((c) => docker.selected.includes(c.id));

  /** Opens a container's web UI in its own window (D-147). */
  function openWeb(c: ContainerView) {
    const h = docker.host;
    const t = c.web[0];
    if (h && t) void webui.open(h.id, targetUrl(t, h.hostname), c.service ?? c.name);
  }
  const webBusy = (c: ContainerView) => {
    const h = docker.host;
    return !!h && !!c.web[0] && webui.isBusy(h.id, targetUrl(c.web[0], h.hostname));
  };

  function shell(c: ContainerView) {
    if (docker.hostId === null) return;
    terms.openContainer(docker.hostId, { id: c.id, name: c.name });
  }

  function onRowKey(e: KeyboardEvent, c: ContainerView) {
    if (e.key === "Enter") docker.detailId = c.id;
    else if (e.key === " ") {
      e.preventDefault();
      docker.toggle(c.id);
    }
  }

  const sel = $derived(docker.selectedViews);
  const bulk: DockerAction[] = [
    { kind: "start" },
    { kind: "stop" },
    { kind: "restart" },
    { kind: "pause" },
    { kind: "unpause" },
    { kind: "kill" },
    { kind: "remove", force: false },
  ];
</script>

<div class="docker" class:with-panel={!!docker.detail && docker.tab === "containers"}>
  <div class="toolbar">
    <label class="hostpick">
      <span class="label">HOST</span>
      <select
        class="input"
        value={docker.hostId ?? ""}
        onchange={(e) => docker.select(Number((e.currentTarget as HTMLSelectElement).value))}
        aria-label="Host"
      >
        {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
    </label>
    <div class="tabstrip" role="tablist" aria-label="Docker view">
      <button type="button" role="tab" data-guide="docker-containers" class:on={docker.tab === "containers"} aria-selected={docker.tab === "containers"} onclick={() => (docker.tab = "containers")}
        >CONTAINERS</button
      >
      <button type="button" role="tab" data-guide="docker-stacks" class:on={docker.tab === "stacks"} aria-selected={docker.tab === "stacks"} onclick={() => (docker.tab = "stacks")}
        >STACKS</button
      >
      <button type="button" role="tab" data-guide="docker-routes-tab" class:on={docker.tab === "routes"} aria-selected={docker.tab === "routes"} onclick={() => (docker.tab = "routes")}
        >ROUTES</button
      >
    </div>
    {#if docker.info}
      <span class="engine" title={docker.info.route}>ENGINE {docker.info.version} · API {docker.info.apiVersion}</span>
    {/if}
    {#if docker.loaded}
      <span class="counts"><span class="mint">{docker.running}</span> / {docker.containers.length} RUNNING</span>
    {/if}
    <span class="spacer"></span>
    <input
      class="input filter"
      type="search"
      placeholder={docker.tab === "stacks" ? "Filter stacks" : docker.tab === "routes" ? "Filter domains" : "Filter name, image, project"}
      bind:value={docker.filter}
      aria-label="Filter"
    />
    {#if docker.tab === "containers"}<label class="chk"><input type="checkbox" bind:checked={docker.showStopped} /> stopped</label>{/if}
    <button type="button" class="btn ghost tiny" onclick={() => docker.refresh()} disabled={docker.loading}>
      {docker.loading ? "…" : "Refresh"}
    </button>
  </div>

  {#if sel.length && docker.tab === "containers"}
    <div class="selbar" role="toolbar" aria-label="Selected containers">
      <span class="count">{sel.length} SELECTED</span>
      {#each bulk as a (a.kind)}
        <button
          type="button"
          class="btn tiny"
          class:danger={a.kind === "kill" || a.kind === "remove"}
          class:ghost={a.kind !== "kill" && a.kind !== "remove"}
          onclick={() => docker.request(a, sel)}>{actionLabel(a)}</button
        >
      {/each}
      <span class="spacer"></span>
      <button type="button" class="btn ghost tiny" onclick={() => (docker.selected = [])}>Clear</button>
    </div>
  {/if}
  {#if docker.notice}
    <p class="notice {docker.notice.tone}" role="status">
      <span use:scramble={docker.notice.text}></span>
      <button type="button" class="x" aria-label="Dismiss" onclick={() => (docker.notice = null)}>×</button>
    </p>
  {/if}

  <div class="split">
    <div class="list" role="grid" aria-label="Containers" aria-multiselectable="true">
      {#if !hosts.hosts.length}
        <div class="empty">
          <p class="label">NO HOSTS</p>
          <p class="muted">Add a host first, then its containers show up here.</p>
          <button type="button" class="btn primary" onclick={() => (hosts.editing = 0)}>Add host</button>
        </div>
      {:else if docker.prompt}
        <div class="overlay">
          {#if docker.prompt.kind === "hostKeyUnknown" || docker.prompt.kind === "hostKeyChanged"}
            <HostKeyPrompt prompt={docker.prompt} onaccept={() => docker.acceptHostKey()} oncancel={() => docker.cancelPrompt()} />
          {:else if docker.prompt.kind === "passwordRequired" || docker.prompt.kind === "passphraseRequired"}
            <AuthPrompt
              prompt={docker.prompt}
              canRemember={ui.keychain}
              onsubmit={(s, r) => docker.provideSecret(s, r)}
              oncancel={() => docker.cancelPrompt()}
            />
          {/if}
        </div>
      {:else if docker.problem}
        <div class="empty">
          {#if docker.problem.kind === "permissionDenied"}
            <p class="label warn">NO ACCESS TO DOCKER</p>
            <p>
              <strong>{docker.problem.user}</strong> can't use <code>{docker.problem.socket}</code> on {hostName}.
              Add the user to the <code>docker</code> group on the server, then retry:
            </p>
            <pre class="cmd">sudo usermod -aG docker {docker.problem.user}</pre>
            <p class="muted">
              Group changes apply to new logins, so close this host's terminals first (Nightgrid reconnects on retry).
              Being in the docker group is root-equivalent on that host.
            </p>
          {:else}
            <p class="label warn">DOCKER NOT FOUND</p>
            <p class="muted">No Docker socket and no <code>docker</code> command on {hostName}.</p>
          {/if}
          <button type="button" class="btn primary" onclick={() => docker.retry()}>Retry</button>
        </div>
      {:else if docker.error}
        <div class="empty">
          <p class="error-text" role="alert" use:scramble={docker.error}></p>
          <button type="button" class="btn primary" onclick={() => docker.retry()}>Retry</button>
        </div>
      {:else if !docker.loaded}
        <p class="muted pad">Connecting to Docker on {hostName}…</p>
      {:else if docker.tab === "stacks"}
        <StacksPanel />
      {:else if docker.tab === "routes"}
        <RoutesPanel />
      {:else if !docker.groups.length}
        <p class="muted pad">{docker.containers.length ? "No containers match." : "No containers on this host."}</p>
      {:else}
        <div class="head row" role="row">
          <span></span>
          <span role="columnheader">NAME</span>
          <span role="columnheader">IMAGE</span>
          <span role="columnheader">STATUS</span>
          <span role="columnheader">PORTS</span>
          <span role="columnheader" class="num">CPU</span>
          <span role="columnheader" class="num">MEM</span>
          <span></span>
        </div>
        {#each docker.groups as g (g.key)}
          {@const open = !docker.collapsed.includes(g.key)}
          <div class="group" role="row">
            <input
              type="checkbox"
              checked={groupAll(g)}
              indeterminate={groupSome(g)}
              onchange={() => docker.toggleGroup(g)}
              aria-label="Select all in {g.label}"
            />
            <button type="button" class="gname" aria-expanded={open} onclick={() => docker.toggleCollapsed(g.key)}>
              <span class="chev" aria-hidden="true">{open ? "▾" : "▸"}</span>
              <span class="label">{g.key ? "▣ " : ""}{g.label}</span>
              <span class="gcount"><span class:mint={g.running > 0}>{g.running}</span>/{g.containers.length}</span>
              {#if g.dir}<span class="muted dir" title="Compose working directory">{g.dir}</span>{/if}
            </button>
            {#if g.key}
              <span class="gacts">
                <button type="button" class="act" title="Start all in {g.label}" onclick={() => docker.request({ kind: "start" }, g.containers)}>▶</button>
                <button type="button" class="act" title="Restart all in {g.label}" onclick={() => docker.request({ kind: "restart" }, g.containers)}>↻</button>
                <button type="button" class="act" title="Stop all in {g.label}" onclick={() => docker.request({ kind: "stop" }, g.containers.filter(isUp))}>■</button>
              </span>
            {/if}
          </div>
          {#if open}
            {#each g.containers as c (c.id)}
              {@const busy = docker.busy.includes(c.id)}
              <div
                class="row c {c.state}"
                class:selected={docker.selected.includes(c.id)}
                class:current={docker.detailId === c.id}
                class:busy
                role="row"
                tabindex="0"
                aria-selected={docker.selected.includes(c.id)}
                onclick={(e) => {
                  if (!(e.target as Element).closest("input, button")) docker.detailId = c.id;
                }}
                onkeydown={(e) => onRowKey(e, c)}
              >
                <span role="gridcell">
                  <input type="checkbox" checked={docker.selected.includes(c.id)} onchange={() => docker.toggle(c.id)} aria-label="Select {c.name}" />
                </span>
                <span class="name" role="gridcell" title={c.name}>
                  <span class="dot" aria-hidden="true"></span>{c.service && c.project ? c.service : c.name}
                  {#if c.service && c.project && c.service !== c.name}<span class="muted cname">{c.name}</span>{/if}
                </span>
                <span class="image muted" role="gridcell" title={c.image}>{c.image}</span>
                <span class="status" role="gridcell">
                  <span class="state">{c.state.toUpperCase()}</span>
                  {#if c.health}<span class="health {c.health}">{c.health.toUpperCase()}</span>{/if}
                  <span class="muted up">{uptime(c)}</span>
                </span>
                <span class="ports data" role="gridcell" title={ports(c)}>{ports(c)}</span>
                <span class="num data" role="gridcell">{cpu(c)}</span>
                <span class="num data" role="gridcell" title={memTitle(c)}>{mem(c)}</span>
                <span class="acts" role="gridcell">
                  {#if busy}
                    <span class="spin" aria-label="Working">◌</span>
                  {:else if isUp(c)}
                    {#if c.web.length && c.state === "running"}
                      <button
                        type="button"
                        class="act web"
                        title="Open the web UI ({targetLabel(c.web[0])}) in a Nightgrid window"
                        aria-label="Open the web UI of {c.name}"
                        data-guide="docker-webui"
                        disabled={webBusy(c)}
                        onclick={() => openWeb(c)}>{webBusy(c) ? "◌" : "⧉"}</button
                      >
                    {/if}
                    <button type="button" class="act" title="Shell" aria-label="Shell in {c.name}" onclick={() => shell(c)} disabled={c.state !== "running"}>›_</button>
                    <button type="button" class="act" title="Restart" aria-label="Restart {c.name}" onclick={() => docker.request({ kind: "restart" }, [c])}>↻</button>
                    <button type="button" class="act" title="Stop" aria-label="Stop {c.name}" onclick={() => docker.request({ kind: "stop" }, [c])}>■</button>
                  {:else if c.state === "paused"}
                    <button type="button" class="act" title="Unpause" aria-label="Unpause {c.name}" onclick={() => docker.request({ kind: "unpause" }, [c])}>❚▶</button>
                  {:else}
                    <button type="button" class="act" title="Start" aria-label="Start {c.name}" onclick={() => docker.request({ kind: "start" }, [c])}>▶</button>
                    <button type="button" class="act" title="Remove" aria-label="Remove {c.name}" onclick={() => docker.request({ kind: "remove", force: false }, [c])}>⌫</button>
                  {/if}
                </span>
              </div>
            {/each}
          {/if}
        {/each}
      {/if}
    </div>

    {#if docker.detail && docker.hostId !== null && docker.tab === "containers"}
      <Resizer key="dockerSide" axis="x" invert label="Resize container panel" />
      <div class="side" style:flex-basis="{layout.get('dockerSide')}%">
        <ContainerPanel hostId={docker.hostId} container={docker.detail} onclose={() => (docker.detailId = null)} />
      </div>
    {/if}
  </div>
</div>

<Modal
  open={!!pending}
  title={pending ? `${actionLabel(pending.action).toUpperCase()} CONTAINER${pending.targets.length > 1 ? "S" : ""}` : ""}
  tone="danger"
  width={560}
  onclose={() => (docker.pending = null)}
>
  {#if pending}
    <p>
      {actionLabel(pending.action)}
      {pending.targets.length === 1 ? "this container" : `these ${pending.targets.length} containers`} on <strong>{hostName}</strong>?
    </p>
    <ul class="targets">
      {#each pending.targets.slice(0, 15) as t (t.id)}
        <li><code>{t.name}</code> <span class="muted">{t.image} · {t.state}</span></li>
      {/each}
      {#if pending.targets.length > 15}<li class="muted">…and {pending.targets.length - 15} more</li>{/if}
    </ul>
    <p class="muted">{consequence[pending.action.kind]}</p>
    {#if pending.action.kind === "remove" && runningTargets.length}
      <label class="chk force">
        <input type="checkbox" bind:checked={forceRemove} />
        Force: kill {runningTargets.length === 1 ? runningTargets[0].name : `the ${runningTargets.length} running containers`} first
      </label>
    {/if}
    {#if needsTyping}
      <label class="field">
        <span>Type <strong>{hostName}</strong> to confirm</span>
        <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
      </label>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (docker.pending = null)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      disabled={!confirmOk}
      onclick={() => pending && pendingAction && docker.run(pendingAction, pending.targets)}
    >
      {pendingAction ? actionLabel(pendingAction) : ""}
      {pending?.targets.length === 1 ? pending.targets[0].name : `${pending?.targets.length ?? 0} containers`}
    </button>
  {/snippet}
</Modal>

<style>
  .docker {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .toolbar,
  .selbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .selbar {
    padding: 4px 8px;
    border: 1px solid var(--magenta);
    box-shadow: var(--glow-active);
  }
  .selbar .count {
    color: var(--magenta);
    letter-spacing: 0.12em;
  }
  .hostpick {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .engine,
  .counts {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .filter {
    width: 220px;
  }
  .chk {
    display: flex;
    gap: 4px;
    align-items: center;
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .mint {
    color: var(--mint);
  }
  .notice {
    margin: 0;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
  }
  .notice.ok {
    color: var(--mint);
    border-color: color-mix(in srgb, var(--mint) 50%, var(--line));
  }
  .notice.err {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 50%, var(--line));
  }
  .x {
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .list {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .side {
    flex: 0 0 52%;
    min-width: 320px;
    min-height: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 26px minmax(110px, 1.3fr) minmax(0, 1.2fr) minmax(140px, 1.3fr) minmax(0, 0.8fr) 56px 70px 120px;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    min-height: 28px;
    font-size: var(--fs-sm);
    border-bottom: 1px solid color-mix(in srgb, var(--line) 45%, transparent);
  }
  /* Next to the detail panel: drop image and ports. */
  .with-panel .row {
    grid-template-columns: 26px minmax(90px, 1fr) minmax(90px, 1fr) 50px 64px 80px;
  }
  /* The panel has its own Web UI button. */
  .with-panel .act.web {
    display: none;
  }
  .with-panel .row > :nth-child(3),
  .with-panel .row > :nth-child(5) {
    display: none;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    min-height: 24px;
  }
  .num {
    text-align: right;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    background: color-mix(in srgb, var(--bg-1) 70%, transparent);
    border-bottom: 1px solid var(--line);
    border-top: 1px solid var(--line);
  }
  .gname {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    background: transparent;
    color: var(--text-0);
    cursor: pointer;
    padding: 2px 0;
    min-width: 0;
    flex: 1;
    text-align: left;
  }
  .gname .label {
    color: var(--cyan);
  }
  .gcount {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .dir {
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    color: var(--text-1);
    width: 10px;
  }
  .gacts,
  .acts {
    display: flex;
    gap: 2px;
    justify-content: flex-end;
  }
  .act {
    min-width: 24px;
    height: 20px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: 10px;
    padding: 0 3px;
  }
  .act:hover:not(:disabled) {
    color: var(--text-0);
    border-color: var(--text-1);
  }
  .act:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .c {
    cursor: pointer;
  }
  .c:hover {
    background: color-mix(in srgb, var(--line) 25%, transparent);
  }
  .c:focus {
    outline: none;
  }
  .c:focus-visible {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .c.selected {
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .c.current {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .c.busy {
    opacity: 0.6;
  }
  .name,
  .image,
  .ports,
  .up {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    color: var(--text-0);
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .cname {
    font-size: var(--fs-xs);
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    background: var(--text-1);
    opacity: 0.6;
  }
  .running .dot {
    background: var(--mint);
    opacity: 1;
    box-shadow: 0 0 6px color-mix(in srgb, var(--mint) 60%, transparent);
  }
  .paused .dot,
  .restarting .dot {
    background: var(--warn);
    opacity: 1;
  }
  .dead .dot {
    background: var(--danger);
    opacity: 1;
  }
  .image {
    font-size: var(--fs-xs);
  }
  .status {
    display: flex;
    gap: 6px;
    align-items: center;
    min-width: 0;
    font-size: var(--fs-xs);
  }
  .state {
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .running .state {
    color: var(--mint);
  }
  .paused .state,
  .restarting .state {
    color: var(--warn);
  }
  .health {
    border: 1px solid currentColor;
    padding: 0 4px;
    font-size: 9px;
    letter-spacing: 0.1em;
  }
  .health.healthy {
    color: var(--mint);
  }
  .health.unhealthy {
    color: var(--danger);
  }
  .health.starting {
    color: var(--warn);
  }
  .data {
    color: var(--cyan);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .spin {
    color: var(--magenta);
    animation: spin 1s linear infinite;
    display: inline-block;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .empty {
    padding: 24px;
    display: grid;
    gap: 10px;
    justify-items: start;
    max-width: 640px;
  }
  .empty p {
    margin: 0;
  }
  .warn {
    color: var(--warn);
  }
  .cmd {
    margin: 0;
    padding: 8px 10px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    color: var(--cyan);
    font-family: var(--font-mono);
    user-select: all;
  }
  code {
    color: var(--cyan);
  }
  .pad {
    padding: 14px;
    margin: 0;
  }
  .overlay {
    padding: 24px;
    display: grid;
    place-items: center;
  }
  .targets {
    margin: 8px 0;
    padding-left: 18px;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-sm);
  }
  .field {
    display: grid;
    gap: 6px;
    margin-top: 10px;
    font-size: var(--fs-sm);
  }
  .force {
    margin-top: 8px;
    color: var(--text-0);
    font-size: var(--fs-sm);
  }
</style>
