<script lang="ts">
  /** A guest's hardware and options (M10r part 4): edit, grow, attach, detach, pass through. */
  import {
    cdromValue,
    cfgValue,
    changed,
    CPU_TYPES,
    deviceLabel,
    DISK_BUSES,
    diskValue,
    freeKey,
    groupMates,
    isQ35,
    NIC_MODELS,
    nicForm,
    nicValue,
    pciValue,
    startupOf,
    startupText,
    validCtIp,
    validIpv4,
    validMountPath,
    type NicForm,
    type PveDisk,
    type PveNic,
  } from "../../proxmox";
  import { proxmox } from "../../stores/proxmox.svelte";

  const d = $derived(proxmox.detail);
  const g = $derived(d?.guest);
  const c = $derived(d?.config ?? null);
  const vm = $derived(g?.kind === "qemu");
  const bridges = $derived(proxmox.createInfo?.bridges.map((b) => b.name) ?? []);
  const isos = $derived(proxmox.createInfo?.isos ?? []);

  $effect(() => {
    if (!proxmox.createInfo && !proxmox.createInfoError) void proxmox.loadCreateInfo();
  });

  // Forms follow the config each time it's read again.
  let cores = $state(1);
  let sockets = $state(1);
  let cpu = $state("");
  let memory = $state(512);
  let balloon = $state(0);
  let swap = $state(0);
  let onboot = $state(false);
  let order = $state<number | null>(null);
  let upDelay = $state<number | null>(null);
  let protection = $state(false);
  let tags = $state("");
  let description = $state("");
  let seen = "";
  $effect(() => {
    if (c && c.digest !== seen) {
      seen = c.digest;
      cores = Number(cfgValue(c, "cores") ?? 1);
      sockets = Number(cfgValue(c, "sockets") ?? 1);
      cpu = cfgValue(c, "cpu") ?? (vm ? "kvm64" : "");
      memory = Number(cfgValue(c, "memory") ?? 512);
      balloon = Number(cfgValue(c, "balloon") ?? memory);
      swap = Number(cfgValue(c, "swap") ?? 0);
      onboot = cfgValue(c, "onboot") === "1";
      const st = startupOf(cfgValue(c, "startup"));
      order = st.order;
      upDelay = st.up;
      protection = cfgValue(c, "protection") === "1";
      tags = (cfgValue(c, "tags") ?? "").split(/[;, ]/).filter(Boolean).join(", ");
      description = cfgValue(c, "description") ?? "";
      editNic = null;
      grow = null;
    }
  });

  const cpuWant = $derived<Record<string, string | null>>(
    vm
      ? { cores: String(cores), sockets: String(sockets), cpu: cpu.trim() || null, memory: String(memory), balloon: balloon >= memory ? null : String(balloon) }
      : { cores: String(cores), memory: String(memory), swap: String(swap) },
  );
  const cpuChange = $derived(changed(c, cpuWant));
  const cpuOk = $derived(cores >= 1 && cores <= 512 && memory >= 16 && (!vm || (sockets >= 1 && sockets <= 8 && balloon >= 0)) && swap >= 0);
  const tagText = $derived(
    tags
      .split(/[;, ]+/)
      .map((t) => t.trim().toLowerCase())
      .filter(Boolean)
      .join(";"),
  );
  const tagsOk = $derived(tagText.split(";").every((t) => !t || /^[a-z0-9_][a-z0-9_.+-]*$/.test(t)));
  const optWant = $derived({
    onboot: onboot ? "1" : null,
    startup: startupText(order, upDelay),
    protection: protection ? "1" : null,
    tags: tagText || null,
    description: description.trim() ? description : null,
  });
  const optChange = $derived(changed(c, optWant));

  function pendingOf(key: string): string | null {
    const e = c?.entries.find((x) => x.key === key);
    if (!e) return null;
    if (e.delete) return "removed after restart";
    return e.pending != null ? `${e.pending} after restart` : null;
  }

  async function apply(what: string, ch: { set: { key: string; value: string }[]; remove: string[] }) {
    await proxmox.applyConfig(what, ch.set, ch.remove).catch(() => {});
  }

  // Disks.
  let grow = $state<{ key: string; gb: number } | null>(null);
  let addBus = $state("scsi");
  let addStore = $state("");
  let addGb = $state(32);
  let addPath = $state("/mnt/data");
  const diskStores = $derived(proxmox.storagesFor(vm ? "images" : "rootdir"));
  $effect(() => {
    if (diskStores.length && !diskStores.includes(addStore)) addStore = diskStores[0];
  });
  const addKey = $derived(vm ? freeKey(c, addBus, DISK_BUSES.find((b) => b.id === addBus)?.max ?? 30) : freeKey(c, "mp", 255));
  const addOk = $derived(!!addKey && !!addStore && addGb >= 1 && (vm || validMountPath(addPath)));
  const realDisks = $derived((c?.disks ?? []).filter((x) => !x.cdrom));
  const cdroms = $derived((c?.disks ?? []).filter((x) => x.cdrom));
  const name = $derived(g ? g.name || String(g.vmid) : "");

  function detach(x: PveDisk) {
    if (!g) return;
    proxmox.askConfig({
      title: vm ? "DETACH DISK" : "REMOVE MOUNT POINT",
      body: vm
        ? `Detach ${x.key} (${x.volid}, ${x.size}) from ${name}? The disk stays on ${x.volid.split(":")[0]} as an unused disk, so it can be attached again or deleted later.${g.status === "running" ? " A running VM may need a restart to let go of it." : ""}`
        : `Remove mount point ${x.key} (${x.mountpoint}) from ${name}? Its volume (${x.volid}) stays as an unused disk.`,
      button: `Detach ${x.key}`,
      remove: [x.key],
    });
  }

  function deleteUnused(key: string, volid: string) {
    proxmox.askConfig({
      title: "DELETE DISK",
      body: `Delete the unused disk ${volid} of ${name}? Its data is erased from the storage and can't be recovered.`,
      button: `Delete ${volid}`,
      typed: key,
      remove: [key],
    });
  }

  function attachUnused(volid: string) {
    const key = vm ? freeKey(c, "scsi", 30) : freeKey(c, "mp", 255);
    if (!key) return;
    if (!vm) return; // a container mount point needs a path: use Add with the volume instead.
    void apply(`Attach ${volid} as ${key}`, { set: [{ key, value: `${volid},discard=on,iothread=1` }], remove: [] });
  }

  // Network.
  let editNic = $state<{ key: string; existing: PveNic | null; f: NicForm } | null>(null);
  function openNic(n: PveNic | null) {
    const key = n?.key ?? freeKey(c, "net", 31);
    if (!key) return;
    editNic = { key, existing: n, f: nicForm(n, bridges[0] ?? "vmbr0") };
  }
  const nicOk = $derived(
    !!editNic &&
      /^[A-Za-z][A-Za-z0-9_.]{0,14}$/.test(editNic.f.bridge) &&
      (editNic.f.vlan === null || (editNic.f.vlan >= 1 && editNic.f.vlan <= 4094)) &&
      (vm || (validCtIp(editNic.f.ip) && (!editNic.f.gateway || (editNic.f.ip !== "dhcp" && validIpv4(editNic.f.gateway))))),
  );
  function saveNic() {
    if (!editNic || !g || !nicOk) return;
    const e = editNic;
    const value = nicValue(g.kind, e.key, e.existing?.value ?? null, e.f);
    void apply(`${e.existing ? "Change" : "Add"} ${e.key}`, { set: [{ key: e.key, value }], remove: [] }).then(() => (editNic = null));
  }
  function removeNic(n: PveNic) {
    proxmox.askConfig({
      title: "REMOVE NETWORK DEVICE",
      body: `Remove ${n.key} (${n.mac || n.model}, on ${n.bridge}${n.vlan ? ` VLAN ${n.vlan}` : ""}) from ${name}? The guest loses that connection${g?.status === "running" ? " right away" : ""}.`,
      button: `Remove ${n.key}`,
      remove: [n.key],
    });
  }

  // Passthrough.
  let pciPick = $state("");
  let allFunctions = $state(true);
  let pcie = $state(true);
  let usbPick = $state("");
  const dev = $derived(d?.devices ?? null);
  const mates = $derived(dev && pciPick ? groupMates(dev.pci, pciPick) : []);
  function addPci() {
    const key = freeKey(c, "hostpci", 15);
    if (!key || !pciPick) return;
    void apply(`Pass ${pciPick} through`, { set: [{ key, value: pciValue(pciPick, allFunctions, pcie && isQ35(c)) }], remove: [] }).then(() => (pciPick = ""));
  }
  function addUsb() {
    const key = freeKey(c, "usb", 14);
    if (!key || !usbPick) return;
    void apply(`Pass USB ${usbPick} through`, { set: [{ key, value: `host=${usbPick}` }], remove: [] }).then(() => (usbPick = ""));
  }
  function removeDevice(key: string, label: string) {
    proxmox.askConfig({ title: "REMOVE DEVICE", body: `Stop passing ${label} through to ${name}? The guest loses the device${g?.status === "running" ? " (PCI devices at the next restart)" : ""}.`, button: `Remove ${key}`, remove: [key] });
  }
</script>

{#if d && g}
  {#if d.configError}<p class="error-text small pre">{d.configError}</p>{/if}
  {#if !c}
    <p class="muted small">{d.configError ? "" : "Reading…"}</p>
  {:else}
    <div class="hw" data-guide="proxmox-hardware-panel">
      {#if c.pending}
        <p class="pend small">
          Some changes wait for the next restart (marked below).
          {#if g.status === "running"}<button type="button" class="btn tiny ghost" onclick={() => proxmox.request(g, "reboot")}>Reboot {name}…</button>{/if}
        </p>
      {/if}

      <h4>PROCESSOR & MEMORY</h4>
      <div class="grid">
        <label class="field"><span class="label">CORES</span><input class="input" type="number" min="1" max="512" bind:value={cores} />{#if pendingOf("cores")}<span class="pend small">{pendingOf("cores")}</span>{/if}</label>
        {#if vm}
          <label class="field"><span class="label">SOCKETS</span><input class="input" type="number" min="1" max="8" bind:value={sockets} /></label>
          <label class="field"
            ><span class="label">CPU TYPE</span>
            <input class="input mono" list="pve-cpu-types" bind:value={cpu} spellcheck="false" />
            <datalist id="pve-cpu-types">{#each CPU_TYPES as t (t)}<option value={t}></option>{/each}</datalist>
          </label>
        {/if}
        <label class="field"><span class="label">MEMORY (MiB)</span><input class="input" type="number" min="16" step="256" bind:value={memory} />{#if pendingOf("memory")}<span class="pend small">{pendingOf("memory")}</span>{/if}</label>
        {#if vm}
          <label class="field" title="Minimum memory with ballooning; equal to memory turns ballooning off, 0 removes the balloon device"><span class="label">BALLOON MIN</span><input class="input" type="number" min="0" step="256" bind:value={balloon} /></label>
        {:else}
          <label class="field"><span class="label">SWAP (MiB)</span><input class="input" type="number" min="0" step="256" bind:value={swap} />{#if pendingOf("swap")}<span class="pend small">{pendingOf("swap")}</span>{/if}</label>
        {/if}
      </div>
      <div class="acts">
        {#if vm && cpu === "host"}<span class="muted small">"host" passes the node's exact CPU: fastest, but live migration needs identical CPUs.</span>{/if}
        <span class="sp"></span>
        <button type="button" class="btn tiny primary" disabled={!cpuOk || (!cpuChange.set.length && !cpuChange.remove.length)} onclick={() => apply("Processor and memory", cpuChange)}>Apply</button>
      </div>

      <h4>DISKS</h4>
      <ul class="list">
        {#each realDisks as x (x.key)}
          <li>
            <span class="key mono">{x.key}</span>
            <span class="mono ell" title={x.value}>{x.volid}</span>
            {#if x.mountpoint}<span class="muted small">{x.mountpoint}</span>{/if}
            <span class="data small">{x.size || "--"}</span>
            {#if x.pending}<span class="pend small">after restart</span>{/if}
            {#if grow?.key === x.key}
              <span class="small">+</span><input class="input tiny-n" type="number" min="1" max="65536" bind:value={grow.gb} aria-label="Grow by GB" /><span class="small">GB</span>
              <button type="button" class="btn tiny primary" disabled={!(grow.gb >= 1)} onclick={() => grow && void proxmox.growDisk(grow.key, grow.gb).then(() => (grow = null), () => {})}>Grow</button>
              <button type="button" class="btn tiny ghost" onclick={() => (grow = null)}>Cancel</button>
            {:else if !["efidisk0", "tpmstate0"].includes(x.key)}
              <button type="button" class="btn tiny ghost" title="Disks only grow; shrinking isn't something Proxmox does" onclick={() => (grow = { key: x.key, gb: 8 })}>Grow…</button>
              {#if x.key !== "rootfs"}<button type="button" class="btn tiny ghost" onclick={() => detach(x)}>Detach…</button>{/if}
            {/if}
          </li>
        {/each}
        {#each cdroms as x (x.key)}
          <li>
            <span class="key mono">{x.key}</span>
            <span class="muted small">CD/DVD</span>
            <select class="input grow" aria-label="Disc in {x.key}" value={x.volid === "none" ? "" : x.volid} onchange={(e) => apply(`Change disc in ${x.key}`, { set: [{ key: x.key, value: cdromValue(e.currentTarget.value || null) }], remove: [] })}>
              <option value="">Empty</option>
              {#if x.volid !== "none" && !isos.some((i) => i.volid === x.volid)}<option value={x.volid}>{x.volid}</option>{/if}
              {#each isos as i (i.volid)}<option value={i.volid}>{i.name}</option>{/each}
            </select>
            <button type="button" class="btn tiny ghost" onclick={() => proxmox.askConfig({ title: "REMOVE DRIVE", body: `Remove the CD/DVD drive ${x.key} from ${name}?`, button: `Remove ${x.key}`, remove: [x.key] })}>Remove…</button>
          </li>
        {/each}
        {#each c.unused as u (u.key)}
          <li class="unused">
            <span class="key mono">{u.key}</span>
            <span class="mono ell">{u.value}</span>
            <span class="warn small">UNUSED</span>
            {#if vm}<button type="button" class="btn tiny ghost" onclick={() => attachUnused(u.value ?? "")}>Attach</button>{/if}
            <button type="button" class="btn tiny ghost danger-text" onclick={() => deleteUnused(u.key, u.value ?? u.key)}>Delete…</button>
          </li>
        {/each}
      </ul>
      <div class="acts" data-guide="proxmox-add-disk">
        <span class="small muted">ADD</span>
        {#if vm}
          <select class="input" bind:value={addBus} aria-label="Bus">{#each DISK_BUSES as b (b.id)}<option value={b.id}>{b.id.toUpperCase()}</option>{/each}</select>
        {:else}
          <input class="input mono" bind:value={addPath} aria-label="Mount path" placeholder="/mnt/data" spellcheck="false" />
        {/if}
        <select class="input" bind:value={addStore} aria-label="Storage">{#each diskStores as s (s)}<option value={s}>{s}</option>{/each}</select>
        <input class="input tiny-n" type="number" min="1" bind:value={addGb} aria-label="Size GB" /><span class="small">GB</span>
        {#if vm && !cdroms.length}<button type="button" class="btn tiny ghost" onclick={() => { const k = freeKey(c, "ide", 3); if (k) void apply(`Add CD/DVD drive ${k}`, { set: [{ key: k, value: cdromValue(null) }], remove: [] }); }}>+ CD/DVD</button>{/if}
        <span class="sp"></span>
        <button type="button" class="btn tiny" disabled={!addOk} onclick={() => addKey && apply(`Add ${addKey}`, { set: [{ key: addKey, value: diskValue(g.kind, addBus, addStore, addGb, addPath) }], remove: [] })}>Add {addKey ?? "disk"}</button>
      </div>
      {#if !vm && addPath && !validMountPath(addPath)}<p class="error-text small">An absolute path inside the container, like /mnt/data.</p>{/if}

      <h4>NETWORK</h4>
      <ul class="list">
        {#each c.nics as n (n.key)}
          <li class:gone={n.delete}>
            <span class="key mono">{n.key}</span>
            <span class="muted small">{n.model}</span>
            <span class="mono small">{n.mac || "auto MAC"}</span>
            <span>{n.bridge}{#if n.vlan}<span class="muted"> · VLAN {n.vlan}</span>{/if}</span>
            {#if n.ip}<span class="mono small">{n.ip}</span>{/if}
            {#if n.firewall}<span class="tag">FW</span>{/if}
            {#if n.pending || n.delete}<span class="pend small">{n.delete ? "removed after restart" : "after restart"}</span>{/if}
            <span class="sp"></span>
            <button type="button" class="btn tiny ghost" onclick={() => openNic(n)}>Edit…</button>
            <button type="button" class="btn tiny ghost" onclick={() => removeNic(n)}>Remove…</button>
          </li>
        {/each}
      </ul>
      {#if editNic}
        {@const e = editNic}
        <form class="box" onsubmit={(ev) => (ev.preventDefault(), saveNic())}>
          <strong class="mono small">{e.key}</strong>
          {#if vm}<select class="input" bind:value={e.f.model} aria-label="Model">{#each NIC_MODELS as m (m)}<option value={m}>{m}</option>{/each}</select>{/if}
          <input class="input mono" list="pve-bridges" bind:value={e.f.bridge} aria-label="Bridge" spellcheck="false" />
          <datalist id="pve-bridges">{#each bridges as b (b)}<option value={b}></option>{/each}</datalist>
          <input class="input tiny-n" inputmode="numeric" placeholder="VLAN" value={e.f.vlan ?? ""} oninput={(ev) => (e.f.vlan = ev.currentTarget.value.trim() ? Number(ev.currentTarget.value) : null)} aria-label="VLAN" />
          {#if !vm}
            <input class="input mono" bind:value={e.f.ip} placeholder="dhcp or 192.168.1.50/24" aria-label="IPv4" spellcheck="false" />
            <input class="input mono" bind:value={e.f.gateway} placeholder="gateway" aria-label="Gateway" disabled={e.f.ip === "dhcp"} spellcheck="false" />
          {/if}
          <label class="check"><input type="checkbox" bind:checked={e.f.firewall} /> firewall</label>
          <span class="sp"></span>
          <button type="button" class="btn tiny ghost" onclick={() => (editNic = null)}>Cancel</button>
          <button type="submit" class="btn tiny primary" disabled={!nicOk}>{e.existing ? "Save" : "Add"}</button>
        </form>
        {#if e.existing && vm && e.f.model !== e.existing.model}<p class="muted small">The MAC address stays the same, so DHCP reservations keep working.</p>{/if}
      {:else}
        <div class="acts"><span class="sp"></span><button type="button" class="btn tiny" onclick={() => openNic(null)}>+ Network device</button></div>
      {/if}

      {#if vm}
        <h4>PASSTHROUGH</h4>
        <ul class="list">
          {#each c.devices as x (x.key)}
            {@const label = deviceLabel(x, dev)}
            <li>
              <span class="key mono">{x.key}</span>
              <span class="ell">{label}</span>
              {#if x.pending != null || x.delete}<span class="pend small">after restart</span>{/if}
              <span class="sp"></span>
              <button type="button" class="btn tiny ghost" onclick={() => removeDevice(x.key, label)}>Remove…</button>
            </li>
          {:else}
            <li class="muted small">No PCI or USB devices passed through.</li>
          {/each}
        </ul>
        {#if !dev}
          <div class="acts"><span class="sp"></span><button type="button" class="btn tiny" data-guide="proxmox-passthrough" onclick={() => proxmox.loadDevices()}>List {g.node}'s devices…</button></div>
        {:else}
          {#if dev.iommuOff}<p class="warn small">IOMMU looks off on {g.node}, so PCI passthrough won't work. Turn on VT-d/AMD-Vi in the firmware and add intel_iommu=on (Intel) to the kernel command line.</p>{/if}
          <div class="acts">
            <span class="small muted">PCI</span>
            <select class="input grow" bind:value={pciPick} aria-label="PCI device">
              <option value="">Pick a device…</option>
              {#each dev.pci as p (p.id)}<option value={p.id}>{p.id} · {p.device || p.vendor}{p.iommuGroup >= 0 ? ` · group ${p.iommuGroup}` : ""}</option>{/each}
            </select>
            <label class="check" title="All functions of the device (a GPU's audio too)"><input type="checkbox" bind:checked={allFunctions} /> all functions</label>
            {#if isQ35(c)}<label class="check"><input type="checkbox" bind:checked={pcie} /> PCIe</label>{/if}
            <button type="button" class="btn tiny" disabled={!pciPick || !freeKey(c, "hostpci", 15)} onclick={addPci}>Add</button>
          </div>
          {#if mates.length}<p class="warn small">Same IOMMU group: {mates.map((m) => `${m.id} (${m.device || m.vendor})`).join(", ")}. The node loses those too while the VM runs.</p>{/if}
          <div class="acts">
            <span class="small muted">USB</span>
            <select class="input grow" bind:value={usbPick} aria-label="USB device">
              <option value="">Pick a device…</option>
              {#each dev.usb as u (u.id + u.path)}<option value={u.id}>{u.id} · {u.product || u.manufacturer}</option>{/each}
            </select>
            <button type="button" class="btn tiny" disabled={!usbPick || !freeKey(c, "usb", 14)} onclick={addUsb}>Add</button>
          </div>
        {/if}
      {/if}

      <h4>OPTIONS</h4>
      <div class="grid">
        <label class="check"><input type="checkbox" bind:checked={onboot} /> Start when {g.node} boots</label>
        <label class="field"><span class="label">BOOT ORDER</span><input class="input" type="number" min="0" placeholder="any" value={order ?? ""} oninput={(e) => (order = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} /></label>
        <label class="field" title="Seconds to wait before starting the next guest"><span class="label">THEN WAIT (s)</span><input class="input" type="number" min="0" placeholder="0" value={upDelay ?? ""} oninput={(e) => (upDelay = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} /></label>
        <label class="check" title="Blocks destroying the guest and removing its disks until turned off"><input type="checkbox" bind:checked={protection} /> Protection</label>
      </div>
      <label class="field"><span class="label">TAGS</span><input class="input" bind:value={tags} placeholder="media, prod" spellcheck="false" /></label>
      {#if !tagsOk}<p class="error-text small">Tags: lowercase letters, digits, _ . + -.</p>{/if}
      <label class="field"><span class="label">NOTES</span><textarea class="input" rows="2" maxlength="8000" bind:value={description}></textarea></label>
      <div class="acts">
        <span class="sp"></span>
        <button type="button" class="btn tiny primary" disabled={!tagsOk || (!optChange.set.length && !optChange.remove.length)} onclick={() => apply("Options", optChange)}>Apply</button>
      </div>
    </div>
  {/if}
{/if}

<style>
  .hw {
    max-height: 62vh;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-right: 4px;
  }
  h4 {
    margin: 8px 0 0;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    font-weight: normal;
    border-bottom: 1px solid var(--line);
    padding-bottom: 2px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 8px;
    align-items: end;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .acts,
  .box {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .box {
    border: 1px solid var(--cyan);
    padding: 6px;
  }
  .sp {
    margin-left: auto;
  }
  .grow {
    flex: 1;
    min-width: 160px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .list li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 2px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .list li.gone {
    opacity: 0.6;
  }
  .key {
    min-width: 64px;
    color: var(--cyan);
  }
  .ell {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tiny-n {
    width: 76px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pend {
    color: var(--magenta);
    margin: 0;
  }
  .warn {
    color: var(--warn);
    margin: 0;
  }
  .error-text {
    margin: 0;
  }
  .pre {
    white-space: pre-wrap;
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 4px;
    color: var(--cyan);
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .danger-text {
    color: var(--danger);
  }
  textarea {
    resize: vertical;
  }
  .btn {
    white-space: nowrap;
  }
</style>
