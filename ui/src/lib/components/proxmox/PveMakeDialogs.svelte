<script lang="ts">
  /** Proxmox (M10r part 3): create a VM or container, clone, download an ISO or a CT template. */
  import Modal from "../Modal.svelte";
  import { errorText } from "../../api";
  import { formatBytes, isoNameFromUrl, OSTYPES, validCtIp, validGuestName, validIpv4, validIsoName, validSha256, validSshKeys, validUrl, vmidFree } from "../../proxmox";
  import { proxmox } from "../../stores/proxmox.svelte";

  const m = $derived.by(() => {
    const x = proxmox.make;
    return x && x.kind !== "sandbox" ? x : null;
  });
  const o = $derived(proxmox.overview);
  const info = $derived(proxmox.createInfo);
  const node = $derived(proxmox.localNode ?? "");

  // Shared fields.
  let vmid = $state(0);
  let idTouched = false;
  let name = $state("");
  let cores = $state(2);
  let memoryMb = $state(2048);
  let diskGb = $state(32);
  let storage = $state("");
  let bridge = $state("");
  let vlanText = $state("");
  let start = $state(true);
  let busy = $state(false);
  let err = $state<string | null>(null);
  // VM.
  let iso = $state("");
  let ostype = $state("l26");
  let uefi = $state(false);
  // CT.
  let template = $state("");
  let swapMb = $state(512);
  let ip = $state("dhcp");
  let gateway = $state("");
  let sshKeys = $state("");
  let unprivileged = $state(true);
  let nesting = $state(true);
  // Clone.
  let full = $state(true);
  // ISO.
  let url = $state("");
  let filename = $state("");
  let nameTouched = $state(false);
  let sha = $state("");
  // Appliances.
  let applianceQuery = $state("");

  /** Reset the form each time a dialog opens. */
  let openedFor: unknown = null;
  $effect(() => {
    if (m && m !== openedFor) {
      openedFor = m;
      err = null;
      busy = false;
      name = m.kind === "clone" ? `${m.guest.name || m.guest.vmid}-clone` : "";
      cores = 2;
      memoryMb = m.kind === "ct" ? 1024 : 2048;
      diskGb = m.kind === "ct" ? 8 : 32;
      vlanText = "";
      start = m.kind !== "clone";
      iso = "";
      ostype = "l26";
      uefi = false;
      template = "";
      ip = "dhcp";
      gateway = "";
      sshKeys = "";
      unprivileged = true;
      nesting = true;
      full = m.kind === "clone" ? !m.guest.template : true;
      url = "";
      filename = "";
      nameTouched = false;
      sha = "";
      applianceQuery = "";
      storage = "";
      bridge = "";
      vmid = 0;
      idTouched = false;
      const g = (m.kind === "vm" || m.kind === "ct") && m.preset ? m.preset : null;
      if (g) {
        name = g.name;
        cores = g.cores;
        memoryMb = g.memoryMb;
        diskGb = g.diskGb;
        if (m.kind === "vm") {
          ostype = g.ostype || "l26";
          uefi = g.uefi;
        } else {
          swapMb = g.swapMb;
          unprivileged = g.unprivileged;
          nesting = g.nesting;
        }
      }
    } else if (!m) openedFor = null;
  });

  // A starter names a template by its start (debian-12): pick the newest that matches once the list loads.
  $effect(() => {
    const hint = m?.kind === "ct" && m.preset ? m.preset.template : "";
    if (hint && !template) {
      const match = templates.filter((t) => t.name.split("/").pop()!.startsWith(hint)).sort((a, b) => a.name.localeCompare(b.name)).pop();
      if (match) template = match.volid;
    }
  });

  const content = $derived(m?.kind === "ct" ? "rootdir" : m?.kind === "iso" ? "iso" : m?.kind === "appliance" ? "vztmpl" : "images");
  const stores = $derived(m?.kind === "clone" ? proxmox.storagesFor(m.guest.kind === "lxc" ? "rootdir" : "images") : proxmox.storagesFor(content));
  $effect(() => {
    if (stores.length && !stores.includes(storage)) storage = stores[0];
  });
  $effect(() => {
    if (info && !idTouched) vmid = info.nextId;
    if (info?.bridges.length && !info.bridges.some((b) => b.name === bridge)) bridge = info.bridges[0].name;
  });
  $effect(() => {
    if (!nameTouched) filename = isoNameFromUrl(url);
  });

  const vlan = $derived(vlanText.trim() ? Number(vlanText) : null);
  const vlanOk = $derived(vlan === null || (Number.isInteger(vlan) && vlan >= 1 && vlan <= 4094));
  const idOk = $derived(vmidFree(o, vmid));
  const isos = $derived(info?.isos ?? []);
  const templates = $derived(info?.templates ?? []);
  const ipOk = $derived(validCtIp(ip.trim()));
  const gwOk = $derived(!gateway.trim() || (ip.trim() !== "dhcp" && validIpv4(gateway.trim())));

  const vmOk = $derived(idOk && validGuestName(name) && cores >= 1 && cores <= 128 && memoryMb >= 128 && diskGb >= 1 && !!storage && !!bridge && vlanOk);
  const ctOk = $derived(idOk && validGuestName(name) && !!template && cores >= 1 && memoryMb >= 64 && diskGb >= 1 && !!storage && !!bridge && vlanOk && ipOk && gwOk && validSshKeys(sshKeys));
  const cloneOk = $derived(idOk && validGuestName(name) && (full || (m?.kind === "clone" && m.guest.template)));
  const isoOk = $derived(!!storage && validUrl(url.trim()) && validIsoName(filename) && (!sha.trim() || validSha256(sha.trim())));

  const shownAppliances = $derived(
    (proxmox.appliances ?? []).filter((a) => {
      const q = applianceQuery.trim().toLowerCase();
      return !q || `${a.template} ${a.headline} ${a.os} ${a.section}`.toLowerCase().includes(q);
    }),
  );
  const have = $derived(new Set(templates.map((t) => t.name)));

  async function run(f: () => Promise<void>) {
    busy = true;
    err = null;
    try {
      await f();
    } catch (e) {
      err = errorText(e);
    } finally {
      busy = false;
    }
  }

  function submit() {
    if (!m || busy) return;
    if (m.kind === "vm" && vmOk)
      void run(() => proxmox.createVm({ node, vmid, name, cores, memoryMb, diskStorage: storage, diskGb, iso: iso || null, bridge, vlan, ostype, uefi: uefi || ostype === "win11", start }));
    else if (m.kind === "ct" && ctOk)
      void run(() =>
        proxmox.createCt({ node, vmid, hostname: name, template, storage, diskGb, cores, memoryMb, swapMb, bridge, vlan, ip: ip.trim(), gateway: gateway.trim() || null, sshKeys: sshKeys.trim(), unprivileged, nesting, start }),
      );
    else if (m.kind === "clone" && cloneOk)
      void run(() => proxmox.clone({ node: m.guest.node, kind: m.guest.kind, vmid: m.guest.vmid, newid: vmid, name, full, storage: full && storage ? storage : null }));
    else if (m.kind === "iso" && isoOk) void run(() => proxmox.downloadIso(storage, url.trim(), filename, sha.trim() || null));
  }

  const TITLE = { vm: "NEW VIRTUAL MACHINE", ct: "NEW CONTAINER", clone: "CLONE", iso: "DOWNLOAD ISO", appliance: "CONTAINER TEMPLATES" } as const;
</script>

<Modal open={!!m} title={m ? `${TITLE[m.kind]}${m.kind === "clone" ? ` ${m.guest.name || m.guest.vmid}` : ""}` : ""} width={m?.kind === "appliance" ? 720 : 600} onclose={() => (proxmox.make = null)}>
  {#if m}
    <form class="form" data-guide="proxmox-make" onsubmit={(e) => (e.preventDefault(), submit())}>
      {#if proxmox.createInfoError}<p class="error-text small pre">{proxmox.createInfoError}</p>{/if}
      {#if info?.notes.length}<p class="warn small pre">{info.notes.join("\n")}</p>{/if}
      {#if !node}
        <p class="warn small">This host isn't a Proxmox node; nothing can be created from it.</p>
      {:else if m.kind === "vm" || m.kind === "ct" || m.kind === "clone"}
        <div class="row3">
          <label class="field"
            ><span class="label">ID</span><input class="input mono" type="number" min="100" bind:value={vmid} oninput={() => (idTouched = true)} />
            {#if vmid && !idOk}<span class="error-text small">{o?.guests.some((g) => g.vmid === vmid) ? "in use" : "100 or more"}</span>{/if}
          </label>
          <label class="field span2"
            ><span class="label">{m.kind === "ct" ? "HOSTNAME" : "NAME"}</span><input class="input mono" maxlength="63" bind:value={name} spellcheck="false" placeholder={m.kind === "ct" ? "web-1" : "vm-1"} />
            {#if name && !validGuestName(name)}<span class="error-text small">Letters, digits, - and . only.</span>{/if}
          </label>
        </div>
      {/if}

      {#if node && m.kind === "vm"}
        <div class="row3">
          <label class="field"><span class="label">CORES</span><input class="input" type="number" min="1" max="128" bind:value={cores} /></label>
          <label class="field"><span class="label">MEMORY (MiB)</span><input class="input" type="number" min="128" step="256" bind:value={memoryMb} /></label>
          <label class="field"><span class="label">DISK (GB)</span><input class="input" type="number" min="1" bind:value={diskGb} /></label>
        </div>
        <div class="row3">
          <label class="field"><span class="label">DISK ON</span><select class="input" bind:value={storage}>{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select></label>
          <label class="field span2"
            ><span class="label">INSTALL FROM</span>
            <select class="input" bind:value={iso}>
              <option value="">No ISO (boot from network or attach later)</option>
              {#each isos as v (v.volid)}<option value={v.volid}>{v.name} · {formatBytes(v.size)}</option>{/each}
            </select>
          </label>
        </div>
        {#if !isos.length && info}<p class="muted small">No ISO images yet. <button type="button" class="link" onclick={() => proxmox.open({ kind: "iso" })}>Download one…</button></p>{/if}
        <div class="row3">
          <label class="field"><span class="label">OS</span><select class="input" bind:value={ostype}>{#each OSTYPES as t (t.id)}<option value={t.id}>{t.label}</option>{/each}</select></label>
          <label class="field"><span class="label">BRIDGE</span><select class="input" bind:value={bridge}>{#each info?.bridges ?? [] as b (b.name)}<option value={b.name}>{b.name}{b.cidr ? ` · ${b.cidr}` : ""}</option>{/each}</select></label>
          <label class="field"><span class="label">VLAN</span><input class="input" inputmode="numeric" bind:value={vlanText} placeholder="none" /></label>
        </div>
        <div class="checks">
          <label class="check" title="OVMF firmware with an EFI disk; Windows 11 always gets it, plus a TPM"><input type="checkbox" checked={uefi || ostype === "win11"} disabled={ostype === "win11"} onchange={(e) => (uefi = e.currentTarget.checked)} /> UEFI</label>
          <label class="check"><input type="checkbox" bind:checked={start} /> Start after creating</label>
        </div>
        <p class="muted small">VirtIO SCSI disk with discard and IO thread, VirtIO network, QEMU guest agent on. Install the agent in the guest for clean shutdowns and IP addresses.</p>
      {:else if node && m.kind === "ct"}
        <label class="field"
          ><span class="label">TEMPLATE</span>
          <select class="input" bind:value={template}>
            <option value="">Pick a template…</option>
            {#each templates as v (v.volid)}<option value={v.volid}>{v.name}</option>{/each}
          </select>
        </label>
        {#if !templates.length && info}<p class="muted small">No container templates yet. <button type="button" class="link" onclick={() => proxmox.open({ kind: "appliance" })}>Get one…</button></p>{/if}
        <div class="row4">
          <label class="field"><span class="label">CORES</span><input class="input" type="number" min="1" max="128" bind:value={cores} /></label>
          <label class="field"><span class="label">MEMORY (MiB)</span><input class="input" type="number" min="64" step="256" bind:value={memoryMb} /></label>
          <label class="field"><span class="label">SWAP (MiB)</span><input class="input" type="number" min="0" step="256" bind:value={swapMb} /></label>
          <label class="field"><span class="label">DISK (GB)</span><input class="input" type="number" min="1" bind:value={diskGb} /></label>
        </div>
        <div class="row4">
          <label class="field"><span class="label">DISK ON</span><select class="input" bind:value={storage}>{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select></label>
          <label class="field"><span class="label">BRIDGE</span><select class="input" bind:value={bridge}>{#each info?.bridges ?? [] as b (b.name)}<option value={b.name}>{b.name}</option>{/each}</select></label>
          <label class="field"><span class="label">VLAN</span><input class="input" inputmode="numeric" bind:value={vlanText} placeholder="none" /></label>
          <span></span>
        </div>
        <div class="row3">
          <label class="field span2"><span class="label">IPv4</span><input class="input mono" bind:value={ip} spellcheck="false" placeholder="dhcp or 192.168.1.50/24" /></label>
          <label class="field"><span class="label">GATEWAY</span><input class="input mono" bind:value={gateway} spellcheck="false" placeholder={ip.trim() === "dhcp" ? "from DHCP" : "192.168.1.1"} disabled={ip.trim() === "dhcp"} /></label>
        </div>
        {#if !ipOk}<p class="error-text small">dhcp, or an address with a prefix like 192.168.1.50/24.</p>{:else if !gwOk}<p class="error-text small">The gateway is an IPv4 address, only with a static IP.</p>{/if}
        <label class="field"
          ><span class="label">SSH PUBLIC KEYS</span>
          <textarea class="input mono" rows="3" spellcheck="false" bind:value={sshKeys} placeholder="ssh-ed25519 AAAA… you@laptop"></textarea>
        </label>
        {#if sshKeys.trim() && !validSshKeys(sshKeys)}<p class="error-text small">One OpenSSH public key per line (ssh-ed25519, ssh-rsa, ecdsa-…).</p>{/if}
        <p class="muted small">You log in as root with these keys. Nightgrid never sets a root password; add one inside the container if you need console logins.</p>
        <div class="checks">
          <label class="check" title="Root in the container is not root on the node. Leave on unless something needs it off."><input type="checkbox" bind:checked={unprivileged} /> Unprivileged</label>
          <label class="check" title="Needed by systemd in newer distributions, and by Docker inside the container"><input type="checkbox" bind:checked={nesting} /> Nesting</label>
          <label class="check"><input type="checkbox" bind:checked={start} /> Start after creating</label>
        </div>
      {:else if node && m.kind === "clone"}
        {@const g = m.guest}
        <div class="checks">
          <label class="check"><input type="radio" name="cl" checked={full} onchange={() => (full = true)} /> Full clone: an independent copy of every disk</label>
          <label class="check" title={g.template ? "" : "Only templates can be linked-cloned"}><input type="radio" name="cl" checked={!full} disabled={!g.template} onchange={() => (full = false)} /> Linked clone: shares the template's disks, fast and small</label>
        </div>
        {#if full}
          <label class="field narrow"><span class="label">DISKS ON</span><select class="input" bind:value={storage}>{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select></label>
        {/if}
        {#if g.status === "running" && g.kind === "lxc"}<p class="warn small">Proxmox clones containers only while they're stopped.</p>{/if}
        <p class="muted small">The clone lands on {g.node}, stopped. Change its MAC addresses and hostname-bound settings before running both at once.</p>
      {:else if node && m.kind === "iso"}
        <label class="field"><span class="label">URL</span><input class="input mono" bind:value={url} spellcheck="false" placeholder="https://releases.example.com/os-24.04-amd64.iso" /></label>
        {#if url.trim() && !validUrl(url.trim())}<p class="error-text small">An http(s) address.</p>{/if}
        <div class="row3">
          <label class="field span2"
            ><span class="label">FILE NAME</span><input class="input mono" bind:value={filename} oninput={() => (nameTouched = true)} spellcheck="false" placeholder="name.iso" />
            {#if filename && !validIsoName(filename)}<span class="error-text small">Ends in .iso; letters, digits, . _ - +.</span>{/if}
          </label>
          <label class="field"><span class="label">SAVE TO</span><select class="input" bind:value={storage}>{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select></label>
        </div>
        <label class="field"><span class="label">SHA-256 (OPTIONAL)</span><input class="input mono" bind:value={sha} spellcheck="false" maxlength="64" placeholder="checked after the download; a mismatch deletes the file" /></label>
        {#if sha.trim() && !validSha256(sha.trim())}<p class="error-text small">64 hex characters.</p>{/if}
        <p class="muted small">{node} downloads the file itself; nothing passes through this computer.</p>
      {:else if node && m.kind === "appliance"}
        <div class="row3">
          <input class="input span2" placeholder="Filter: debian, ubuntu, alpine, turnkey…" bind:value={applianceQuery} spellcheck="false" />
          <select class="input" bind:value={storage} aria-label="Save to">{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select>
        </div>
        {#if !proxmox.appliances}
          <p class="muted small">Reading the template list…</p>
        {:else}
          <ul class="apps">
            {#each shownAppliances as a (a.template)}
              <li>
                <span class="mono small">{a.template}</span>
                <span class="muted small desc" title={a.headline}>{a.headline}</span>
                {#if have.has(a.template)}<span class="tag">HAVE IT</span>
                {:else}<button type="button" class="btn tiny ghost" disabled={!storage} onclick={() => void proxmox.downloadAppliance(storage, a.template).catch(() => {})}>Download</button>{/if}
              </li>
            {:else}
              <li class="muted small">Nothing matches.</li>
            {/each}
          </ul>
        {/if}
      {/if}
      {#if node && !stores.length && m.kind !== "appliance"}<p class="warn small">No storage on {node} holds {m.kind === "iso" ? "ISO images" : m.kind === "ct" || (m.kind === "clone" && m.guest.kind === "lxc") ? "container disks" : "VM disks"}.</p>{/if}
      {#if err}<p class="error-text small pre">{err}</p>{/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (proxmox.make = null)}>{m?.kind === "appliance" ? "Close" : "Cancel"}</button>
    {#if m && m.kind !== "appliance"}
      <button
        type="button"
        class="btn primary"
        data-guide="proxmox-make-go"
        disabled={busy || !node || !(m.kind === "vm" ? vmOk : m.kind === "ct" ? ctOk : m.kind === "clone" ? cloneOk : isoOk)}
        onclick={submit}>{busy ? "Starting…" : m.kind === "vm" ? "Create VM" : m.kind === "ct" ? "Create container" : m.kind === "clone" ? "Clone" : "Download"}</button
      >
    {/if}
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .narrow {
    max-width: 240px;
  }
  .row3 {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    align-items: start;
  }
  .row4 {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  .span2 {
    grid-column: span 2;
  }
  .checks {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .mono {
    font-family: var(--font-mono);
  }
  textarea {
    resize: vertical;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pre {
    white-space: pre-wrap;
  }
  .warn {
    color: var(--warn);
  }
  p {
    margin: 0;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
  }
  .apps {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow: auto;
  }
  .apps li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px;
    border-bottom: 1px solid var(--line);
  }
  .desc {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 var(--tag-px);
    color: var(--cyan);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
</style>
