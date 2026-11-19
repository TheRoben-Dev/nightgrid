<script lang="ts">
  /**
   * Stacks from git (M10w): repositories checked out on the host. Check fetches
   * and lists new commits; deploying shows the diff, then moves the checkout to
   * that commit (after a stack snapshot) and brings the stack up. The confirm
   * names the stack, host and commit (rule 5).
   */
  import { scramble } from "../../scramble";
  import { ago, diffLines, repoName, shortSha, validBranch, validDir, validSubdir, validUrl } from "../../gitstacks";
  import { docker } from "../../stores/docker.svelte";
  import { gitStacks } from "../../stores/gitstacks.svelte";
  import type { GitStack } from "../../api";
  import Modal from "../Modal.svelte";

  const hostName = $derived(docker.host?.name ?? "");
  const p = $derived(gitStacks.pending);

  let url = $state("");
  let branch = $state("main");
  let dir = $state("");
  let dirTouched = false;
  let subdir = $state("");
  let formError = $state<string | null>(null);
  let confirmRemove = $state<GitStack | null>(null);

  function onUrl() {
    if (!dirTouched) dir = url.trim() ? `~/stacks/${repoName(url.trim()) || "repo"}` : "";
  }

  async function add(e: SubmitEvent) {
    e.preventDefault();
    const u = url.trim();
    const b = branch.trim();
    const d = dir.trim().replace(/\/+$/, "");
    const s = subdir.trim().replace(/^\/+|\/+$/g, "");
    formError = !validUrl(u)
      ? "Use an https://, ssh:// or user@host:path address."
      : !validBranch(b)
        ? "That isn't a branch name."
        : !validDir(d)
          ? "Use a folder on the host: an absolute path or one starting with ~/."
          : !validSubdir(s)
            ? "Use a folder inside the repository, like stacks/media."
            : null;
    if (formError) return;
    formError = await gitStacks.add({ url: u, branch: b, dir: d, subdir: s });
    if (!formError) {
      url = "";
      dir = "";
      subdir = "";
      dirTouched = false;
    }
  }

  const folder = (g: GitStack) => (g.subdir ? `${g.dir}/${g.subdir}` : g.dir);
</script>

<Modal open={gitStacks.open} title={p ? `DEPLOY · ${folder(p.git)}` : "STACKS FROM GIT"} width={800} onclose={() => gitStacks.close()}>
  {#if p}
    {@const st = gitStacks.checks[p.git.id]?.status}
    <p>
      {p.mode === "rollback" ? "Roll back" : "Update"} <strong>{folder(p.git)}</strong> on <strong>{hostName}</strong> to
      <code>{shortSha(p.target)}</code>
      {#if p.subject}<span class="muted">· {p.subject}</span>{/if}
    </p>
    {#if p.mode === "update" && st?.incoming.length}
      <ul class="commits">
        {#each st.incoming as c (c.id)}<li><code>{shortSha(c.id)}</code> {c.subject} <span class="muted">· {c.author}, {ago(c.at)}</span></li>{/each}
      </ul>
    {/if}
    {#if p.diffError}<p class="error-text" role="alert" use:scramble={p.diffError}></p>{/if}
    {#if p.diff === null && !p.diffError}
      <p class="muted small">Reading the diff on the host…</p>
    {:else if p.diff !== null}
      {@const lines = diffLines(p.diff)}
      {#if lines.length}
        <pre class="diff" aria-label="Diff">{#each lines as l, i (i)}<span class={l.kind}>{l.text}</span>
{/each}</pre>
      {:else}
        <p class="muted small">No file in the stack's folder changes.</p>
      {/if}
    {/if}
    <p class="muted small">
      A stack snapshot is saved first. The checkout moves with <code>git reset --keep</code>, which refuses if files were edited on the host. Then the stack
      is brought up, recreating the services whose configuration changed.
    </p>
  {:else}
    <p class="muted small">
      A repository cloned on {hostName}, with git using the host's own credentials (a deploy key or credential helper there); Nightgrid keeps no token.
      Check fetches new commits; nothing changes on the host until you deploy one.
    </p>
    {#if gitStacks.error}<p class="error-text" role="alert">{gitStacks.error}</p>{/if}
    {#if gitStacks.notice}<p class="notice small" role="status">{gitStacks.notice}</p>{/if}
    {#if confirmRemove}
      <div class="confirm" role="alertdialog" aria-label="Confirm">
        <p>Stop tracking <strong>{folder(confirmRemove)}</strong>? The checkout and the stack stay on {hostName}.</p>
        <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = null)}>Keep</button>
        <button
          type="button"
          class="btn danger tiny"
          onclick={() => {
            const g = confirmRemove;
            confirmRemove = null;
            if (g) void gitStacks.remove(g);
          }}>Stop tracking</button
        >
      </div>
    {/if}

    <ul class="repos">
      {#each gitStacks.list as g (g.id)}
        {@const c = gitStacks.checks[g.id]}
        {@const st = c?.status}
        <li class="repo">
          <div class="head">
            <span class="where"><code>{folder(g)}</code></span>
            <span class="muted small src" title={g.url}>{g.url} · {g.branch}</span>
          </div>
          <div class="state small">
            {#if c?.loading}
              <span class="muted">Fetching…</span>
            {:else if c?.error}
              <span class="error-text">{c.error}</span>
            {:else if st}
              {#if st.behind > 0}
                <span class="tag new">{st.behind} NEW</span>
                <span>{st.incoming[0]?.subject ?? ""}</span>
                <span class="muted">· {ago(st.incoming[0]?.at)}</span>
              {:else}
                <span class="tag ok">UP TO DATE</span>
                <span class="muted"><code>{shortSha(st.head)}</code> {st.current?.subject ?? ""}</span>
              {/if}
              {#if st.ahead > 0}<span class="tag warn" title="Commits on the host that the remote doesn't have">{st.ahead} LOCAL</span>{/if}
              {#if st.dirty.length}<span class="tag warn" title={st.dirty.join("\n")}>EDITED ON HOST</span>{/if}
              {#if st.error}<span class="error-text pre">{st.error}</span>{/if}
              {#if !st.composeFile}<span class="muted">· no compose file in this folder</span>{/if}
            {/if}
            {#if g.deployedAt}<span class="muted">· deployed {ago(g.deployedAt)}</span>{/if}
          </div>
          <div class="acts">
            <button type="button" class="btn ghost tiny" disabled={c?.loading} onclick={() => void gitStacks.check(g)}>Check</button>
            {#if st?.remote && st.behind > 0}
              <button
                type="button"
                class="btn primary tiny"
                data-guide="git-stack-deploy"
                disabled={st.dirty.length > 0}
                title={st.dirty.length ? "Files were edited on the host: commit or undo them there first" : ""}
                onclick={() => st.remote && void gitStacks.prepare(g, st.remote, "update", st.incoming[0]?.subject ?? "")}>Review & deploy…</button
              >
            {/if}
            {#if g.previous && st && st.head !== g.previous}
              <button
                type="button"
                class="btn ghost tiny warnbtn"
                disabled={st.dirty.length > 0}
                onclick={() => g.previous && void gitStacks.prepare(g, g.previous, "rollback", "the previous deploy")}>Roll back to {shortSha(g.previous)}…</button
              >
            {/if}
            <span class="spacer"></span>
            <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = g)}>Stop tracking…</button>
          </div>
        </li>
      {:else}
        <li class="muted small empty">{gitStacks.loading ? "Loading…" : "No repositories on this host yet."}</li>
      {/each}
    </ul>

    <form class="add" onsubmit={add} data-guide="git-stack-add">
      <h4>CLONE A REPOSITORY</h4>
      <div class="grid">
        <label>Repository<input class="input mono" placeholder="git@git.example.com:me/stacks.git" bind:value={url} oninput={onUrl} spellcheck="false" /></label>
        <label>Branch<input class="input mono" bind:value={branch} spellcheck="false" /></label>
        <label
          >Folder on {hostName || "the host"}<input class="input mono" placeholder="~/stacks/stacks" bind:value={dir} oninput={() => (dirTouched = true)} spellcheck="false" /></label
        >
        <label>Stack folder in the repo <span class="muted">(optional)</span><input class="input mono" placeholder="media" bind:value={subdir} spellcheck="false" /></label>
      </div>
      {#if formError}<p class="error-text small pre">{formError}</p>{/if}
      <div class="row">
        <span class="muted small">An existing checkout of the same repository in that folder is adopted as it is.</span>
        <span class="spacer"></span>
        <button type="submit" class="btn primary tiny" disabled={gitStacks.busy || !url.trim()}>{gitStacks.busy ? "Cloning…" : "Clone"}</button>
      </div>
    </form>
  {/if}
  {#snippet footer()}
    {#if p}
      <button type="button" class="btn ghost" onclick={() => (gitStacks.pending = null)}>Back</button>
      <button type="button" class="btn danger" disabled={gitStacks.busy || p.diff === null} onclick={() => void gitStacks.deploy()}>
        {gitStacks.busy ? "Deploying…" : `${p.mode === "rollback" ? "Roll back" : "Deploy"} ${shortSha(p.target)} and up`}
      </button>
    {:else}
      <button type="button" class="btn ghost" onclick={() => gitStacks.close()}>Close</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .notice {
    color: var(--ok);
    margin: 0 0 6px;
  }
  .confirm,
  .row,
  .acts {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .confirm {
    border: 1px solid var(--danger);
    padding: 6px 8px;
    font-size: var(--fs-sm);
    margin-bottom: 8px;
  }
  .confirm p {
    margin: 0;
    flex: 1 1 300px;
  }
  .repos {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    border: 1px solid var(--line);
  }
  .repo {
    padding: 8px;
    border-bottom: 1px solid var(--line);
    display: grid;
    gap: 4px;
  }
  .repo:last-child {
    border-bottom: 0;
  }
  .empty {
    padding: 8px;
  }
  .head {
    display: flex;
    gap: 8px;
    align-items: baseline;
    min-width: 0;
  }
  .where code {
    color: var(--cyan);
  }
  .src {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .pre {
    white-space: pre-wrap;
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 var(--tag-px);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
  .tag.ok {
    color: var(--ok);
    border-color: var(--ok);
  }
  .tag.new {
    color: var(--accent);
    border-color: var(--accent);
  }
  .tag.warn {
    color: var(--warn);
    border-color: var(--warn);
  }
  .warnbtn {
    color: var(--warn);
  }
  .spacer {
    flex: 1;
  }
  .add {
    border: 1px solid var(--line);
    padding: 8px;
  }
  .add h4 {
    margin: 0 0 6px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 6px 10px;
    margin-bottom: 6px;
  }
  .grid label {
    display: grid;
    gap: 2px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .commits {
    margin: 0 0 8px;
    padding-left: 18px;
    font-size: var(--fs-sm);
  }
  .diff {
    margin: 0 0 8px;
    padding: 6px 0;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    max-height: 340px;
    overflow: auto;
  }
  .diff > span {
    display: inline-block;
    min-width: 100%;
    padding: 0 8px;
  }
  .diff .add {
    background: color-mix(in srgb, var(--ok) 12%, transparent);
  }
  .diff .del {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .diff .file {
    color: var(--cyan);
    margin-top: 6px;
  }
  .diff .hunk {
    color: var(--magenta);
  }
  .diff .meta {
    color: var(--text-1);
  }
</style>
