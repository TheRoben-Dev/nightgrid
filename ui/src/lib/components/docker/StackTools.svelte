<script lang="ts">
  /**
   * Stack dialogs (M10f): image updates with changelog links, snapshots with
   * rollback (confirmed naming the services and files it changes).
   */
  import type { ImageStatus, Snapshot, VersionInfo } from "../../api";
  import { openExternal } from "../../clipboard";
  import { docker } from "../../stores/docker.svelte";
  import { availableCount, stackKey, stacks } from "../../stores/stacks.svelte";
  import Modal from "../Modal.svelte";

  const hostName = $derived(docker.host?.name ?? "");
  const uf = $derived(stacks.updatesFor);
  const check = $derived(uf ? stacks.updates[stackKey(uf)] : undefined);
  const sf = $derived(stacks.snapshotsFor);
  const pr = $derived(stacks.pendingRollback);

  const statusWord: Record<ImageStatus, string> = {
    current: "UP TO DATE",
    available: "UPDATE",
    pinned: "PINNED",
    local: "LOCAL BUILD",
    needsLogin: "NEEDS LOGIN",
    unknown: "UNKNOWN",
  };
  const statusHint: Record<ImageStatus, string> = {
    current: "The registry's tag points at the image this host has.",
    available: "The tag moved on the registry; Update would pull it.",
    pinned: "Pinned by digest in the compose file; it never changes on its own.",
    local: "Built on the host or never pulled from a registry.",
    needsLogin: "This registry wants a login; Nightgrid only checks anonymously.",
    unknown: "The registry couldn't be asked.",
  };

  const day = (iso: string | null) => (iso ? iso.slice(0, 10) : "");
  const ver = (v: VersionInfo | null | undefined) => (v ? v.version || (v.revision ? v.revision.slice(0, 8) : "") || day(v.created) || "?" : "?");
  const short = (id: string) => id.replace(/^sha256:/, "").slice(0, 12);
  const when = (ts: number) => new Date(ts * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  const base = (p: string) => p.split("/").pop() ?? p;

  /** Services whose image differs from what the stack runs now (unknown in the UI: all are listed). */
  const rollbackFiles = (s: Snapshot) => s.files.map((f) => f.path);
</script>

<Modal open={!!uf} title={uf ? `UPDATES · ${uf.name}` : ""} width={720} onclose={() => (stacks.updatesFor = null)}>
  {#if uf}
    <p class="muted small">Asks each image's registry whether its tag moved. Nothing is pulled until you choose <strong>Update</strong>.</p>
    {#if check?.loading && !check.list}
      <p class="muted">Asking the registries…</p>
    {:else if check?.error}
      <p class="error-text">{check.error}</p>
    {:else if check?.list}
      {#if !check.list.length}<p class="muted">This stack has no containers to check.</p>{/if}
      <ul class="ups">
        {#each check.list as u (u.image)}
          <li class:avail={u.status === "available"}>
            <div class="top">
              <code class="img">{u.image}</code>
              <span class="badge {u.status}" title={u.error ?? statusHint[u.status]}>{statusWord[u.status]}</span>
            </div>
            <div class="sub muted small">
              {u.services.join(", ")}
              {#if u.status === "available"}
                · <span class="ver">{ver(u.current)}</span> → <span class="ver new">{ver(u.latest)}</span>
                {#if u.latest?.created}<span> (built {day(u.latest.created)})</span>{/if}
              {:else if u.current.version}
                · {u.current.version}
              {/if}
              {#if u.error}<span class="error-text"> · {u.error}</span>{/if}
            </div>
            {#if u.notes || u.source}
              <div class="links">
                {#if u.notes}<button type="button" class="btn ghost tiny" title={u.notes} onclick={() => void openExternal(u.notes!)}
                    >{u.notes.includes("/compare/") ? "What changed" : "Release notes"} ↗</button
                  >{/if}
                {#if u.source && u.source !== u.notes}<button type="button" class="btn ghost tiny" title={u.source} onclick={() => void openExternal(u.source!)}
                    >Project ↗</button
                  >{/if}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#snippet footer()}
    {#if uf}
      <button type="button" class="btn ghost" disabled={check?.loading} onclick={() => uf && stacks.checkUpdates(uf)}>{check?.loading ? "Checking…" : "Check again"}</button>
      <button
        type="button"
        class="btn warnbtn"
        disabled={stacks.busy || !availableCount(check)}
        onclick={() => {
          const s = uf;
          stacks.updatesFor = null;
          if (s) stacks.request(s, "update");
        }}>Update {uf.name}…</button
      >
    {/if}
  {/snippet}
</Modal>

<Modal open={!!sf && !pr} title={sf ? `SNAPSHOTS · ${sf.name}` : ""} width={720} onclose={() => (stacks.snapshotsFor = null)}>
  {#if sf}
    <p class="muted small">
      A snapshot records each service's image and copies the compose files and <code>.env</code> on {hostName} (in
      <code>~/.local/state/nightgrid/snapshots</code>). One is saved before every Update and Rollback; the newest 10 are kept.
    </p>
    {#if stacks.snapshotError}<p class="error-text">{stacks.snapshotError}</p>{/if}
    {#if stacks.snapshotsLoading && !stacks.snapshots.length}
      <p class="muted">Loading…</p>
    {:else if !stacks.snapshots.length}
      <p class="muted">No snapshots yet.</p>
    {:else}
      <ul class="snaps">
        {#each stacks.snapshots as s (s.id)}
          <li>
            <div class="top">
              <span>{when(s.createdAt)}</span>
              <span class="reason">{s.reason.toUpperCase()}</span>
              <span class="spacer"></span>
              <button type="button" class="btn ghost tiny" onclick={() => void stacks.forgetSnapshot(s.id)}>Forget</button>
              <button type="button" class="btn warnbtn tiny" disabled={stacks.busy} onclick={() => {
                  if (sf) stacks.pendingRollback = { stack: sf, snap: s };
                }}>Roll back…</button>
            </div>
            <div class="sub muted small">
              {#each s.services as v (v.service)}<span class="svc"><strong>{v.service}</strong> {v.image} <code>{short(v.imageId)}</code></span>{/each}
              {#if s.files.length}<span class="svc">files: {s.files.map((f) => base(f.path)).join(", ")}</span>{/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn" disabled={stacks.snapshotsLoading} onclick={() => void stacks.takeSnapshot()}>Snapshot now</button>
  {/snippet}
</Modal>

<Modal open={!!pr} title="ROLL BACK STACK" tone="danger" width={600} onclose={() => (stacks.pendingRollback = null)}>
  {#if pr}
    <p>Roll <strong>{pr.stack.name}</strong> on <strong>{hostName}</strong> back to the snapshot from <strong>{when(pr.snap.createdAt)}</strong>?</p>
    {#if rollbackFiles(pr.snap).length}
      <p class="muted small">These files are overwritten with the saved copies:</p>
      <ul class="plain">{#each rollbackFiles(pr.snap) as f (f)}<li><code>{f}</code></li>{/each}</ul>
    {/if}
    <p class="muted small">Services set back to their saved image (pulled by digest if it was removed), then recreated if they changed:</p>
    <ul class="plain">{#each pr.snap.services as v (v.service)}<li><strong>{v.service}</strong> <code>{v.image}</code> → <code>{short(v.imageId)}</code></li>{/each}</ul>
    <p class="muted small">The current state is saved as a snapshot first, so this can be undone the same way.</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (stacks.pendingRollback = null)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => pr && stacks.rollback(pr.stack, pr.snap)}>{pr ? `Roll back ${pr.stack.name}` : ""}</button>
  {/snippet}
</Modal>

<style>
  .small {
    font-size: var(--fs-xs);
  }
  .ups,
  .snaps {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    max-height: 52vh;
    overflow: auto;
    border: 1px solid var(--line);
  }
  .ups li,
  .snaps li {
    padding: 8px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    border-left: 2px solid transparent;
  }
  .ups li.avail {
    border-left-color: var(--warn, var(--yellow));
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .img {
    font-family: var(--font-mono);
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .badge {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    padding: 1px 6px;
    border: 1px solid var(--line);
    color: var(--text-1);
    white-space: nowrap;
  }
  .badge.current {
    color: var(--mint);
    border-color: var(--mint);
  }
  .badge.available {
    color: var(--bg-0);
    background: var(--warn, var(--yellow));
    border-color: var(--warn, var(--yellow));
  }
  .badge.unknown,
  .badge.needsLogin {
    color: var(--text-2);
  }
  .sub {
    margin-top: 3px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
  }
  .ver {
    font-family: var(--font-mono);
    color: var(--text-0);
  }
  .ver.new {
    color: var(--mint);
  }
  .links {
    margin-top: 4px;
    display: flex;
    gap: 6px;
  }
  .reason {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .spacer {
    flex: 1;
  }
  .svc code {
    color: var(--cyan);
  }
  .plain {
    margin: 4px 0 10px;
    padding-left: 18px;
    font-size: var(--fs-sm);
  }
  code {
    font-family: var(--font-mono);
  }
</style>
