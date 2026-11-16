/**
 * DRIFT as a guide (M10q): walkthroughs that spotlight one control at a
 * time and wait for the user to act. Tours never click, type or navigate
 * on their own; a step that needs another view asks the user to go there.
 */
import { SECTIONS, railFor, railPath, type SectionId } from "../sections";
import { GUIDE_TARGETS, guideTarget } from "./guide";

/** What moves a step on. */
export type Wait =
  /** The user clicks the spotlighted control. */
  | { kind: "click" }
  /** The user opens this view. */
  | { kind: "view"; view: SectionId }
  /** Something shows up (a dialog, a panel), by CSS selector. */
  | { kind: "appear"; selector: string }
  /** Something goes away (the user closed a dialog). */
  | { kind: "gone"; selector: string }
  /** The user presses Next. */
  | { kind: "next" };

export interface TourStep {
  /** A `data-guide` id to spotlight; none shows the text on its own. */
  target?: string;
  text: string;
  wait: Wait;
}

export interface Tour {
  id: string;
  title: string;
  /** One line for lists and for DRIFT. */
  what: string;
  /** Views where the tour is a good suggestion ("what can I do here?"). */
  views: SectionId[];
  steps: TourStep[];
}

const go = (view: SectionId, text?: string): TourStep => ({
  target: `rail-${view}`,
  text: text ?? (railFor(view) && (railFor(view)?.members.length ?? 0) > 1 ? `Open ${railPath(view)}: click ${railFor(view)?.label} in the sidebar.` : view === "settings" ? "Open Settings at the bottom of the sidebar." : `Open ${SECTIONS.find((s) => s.id === view)?.label ?? view} in the sidebar.`),
  wait: { kind: "view", view },
});

export const TOURS: Tour[] = [
  {
    id: "first-run",
    title: "Getting around",
    what: "The sidebar, the command palette, adding a host and asking DRIFT",
    views: ["overview"],
    steps: [
      { text: "Hi, I'm DRIFT. This tour points at a few things and waits for you; I never click anything myself. Esc ends it any time.", wait: { kind: "next" } },
      { target: "rail-overview", text: "The sidebar switches views; point at a row to see its shortcut. A row marked +1 holds related views, with tabs beside the title (System has One host and Fleet).", wait: { kind: "next" } },
      { target: "rail-settings", text: "Settings is at the bottom. Its Features tab turns views on or off, so Nightgrid can be as simple or as complete as you like.", wait: { kind: "next" } },
      { target: "palette", text: "The command palette finds hosts, views and actions by name. Try it: click here or press Mod+K.", wait: { kind: "appear", selector: '[aria-label="Command palette"]' } },
      { text: "Type a few letters to filter; Enter runs the highlighted entry. Press Esc to close it and I'll go on.", wait: { kind: "gone", selector: '[aria-label="Command palette"]' } },
      { target: "hosts-add", text: "Add a host here (or import your ~/.ssh/config with the button next to it). Everything else builds on your hosts.", wait: { kind: "next" } },
      { target: "dashboard-edit", text: "The Overview is a dashboard you can rearrange: Edit lets you move, resize and add widgets.", wait: { kind: "next" } },
      { text: "Ask me anything with Mod+J. I can explain logs and errors, check a command before you run it, and replay this tour from Settings → Helper.", wait: { kind: "next" } },
    ],
  },
  {
    id: "add-host",
    title: "Add your first host",
    what: "Save a server, connect to it and open a terminal",
    views: ["overview", "terminal"],
    steps: [
      { target: "hosts-add", text: "Click + to add a host.", wait: { kind: "appear", selector: "dialog[open]" } },
      { text: "Fill in a name, the address and your user. Leave authentication on the SSH agent if your key is loaded there. Save (or cancel) and I'll continue.", wait: { kind: "gone", selector: "dialog[open]" } },
      go("terminal", "Now open Terminal in the sidebar."),
      { text: "Open a shell on the new host from the + tab (or the host's ›_ button on the Overview). The first time, check the host key fingerprint before you accept it.", wait: { kind: "next" } },
    ],
  },
  {
    id: "customize-overview",
    title: "Customize the Overview",
    what: "Move, resize and add widgets, and keep several dashboards",
    views: ["overview"],
    steps: [
      go("overview"),
      { target: "dashboard-edit", text: "Click Edit.", wait: { kind: "click" } },
      { target: "dashboard-add", text: "Add a widget from this list, or drag a title bar to move one and its corner to resize it.", wait: { kind: "next" } },
      { target: "dashboard-presets", text: "Presets swap in a ready-made layout, and you can save yours as one.", wait: { kind: "next" } },
      { target: "dashboard-new", text: "+ New makes another dashboard as a tab, for example one for maintenance days.", wait: { kind: "next" } },
      { target: "dashboard-edit", text: "Click Done when you're happy. Changes are saved as you go.", wait: { kind: "click" } },
    ],
  },
  {
    id: "watch-services",
    title: "Watch your services",
    what: "Add service checks and certificate and domain expiry",
    views: ["services"],
    steps: [
      go("services"),
      { target: "services-add", text: "Add a service by its URL: Nightgrid checks it and shows it on the board.", wait: { kind: "next" } },
      { target: "services-import", text: "Or import them from a Homepage config you already have.", wait: { kind: "next" } },
      { target: "services-expiry", text: "EXPIRY watches certificates, domains and tokens and warns before they run out.", wait: { kind: "next" } },
    ],
  },
  {
    id: "fleet-health",
    title: "Check fleet health",
    what: "Pending updates, disk health and backups across every host",
    views: ["fleet"],
    steps: [
      go("fleet"),
      { target: "fleet-tab-updates", text: "UPDATES lists pending packages and reboots for every host.", wait: { kind: "next" } },
      { target: "fleet-check", text: "Check reads them over SSH. Nothing is installed until you choose to upgrade.", wait: { kind: "next" } },
      { target: "fleet-tab-disks", text: "DISKS shows pools, SMART and how many days until a disk is full. Open it.", wait: { kind: "click" } },
      { target: "fleet-tab-backups", text: "BACKUPS finds restic, borg and rsync jobs and can test-restore a file to prove they work.", wait: { kind: "next" } },
    ],
  },
  {
    id: "back-up-setup",
    title: "Back up your setup",
    what: "Export an encrypted profile with your hosts, settings and dashboards",
    views: ["settings"],
    steps: [
      go("settings"),
      { target: "settings-backup", text: "Profiles hold your hosts, settings, notes and dashboards in one encrypted file. Export one now and keep it somewhere safe.", wait: { kind: "next" } },
      { text: "On a new computer, import that file here. The weekly reminder can be changed in the same place.", wait: { kind: "next" } },
    ],
  },
  {
    id: "vm-screen",
    title: "Open a VM's screen",
    what: "See and use a virtual machine's console over SSH",
    views: ["system", "desktops"],
    steps: [
      go("system"),
      { target: "system-tab-vms", text: "Open the VMS tab (pick a host with libvirt or Proxmox first).", wait: { kind: "click" } },
      { target: "vms-screen", text: "Screen opens a running VM's display in the Desktops view, over the host's SSH connection.", wait: { kind: "next" } },
      { text: "In Desktops, click the screen to type into the guest; the toolbar has Ctrl+Alt+Del and Type text for login screens.", wait: { kind: "next" } },
    ],
  },
];

export const tourById = (id: string): Tour | undefined => TOURS.find((t) => t.id === id);

/** Tours that fit a view, then the rest. */
export function toursFor(view: SectionId): Tour[] {
  return [...TOURS.filter((t) => t.views.includes(view)), ...TOURS.filter((t) => !t.views.includes(view))];
}

/** Every step's target must be a known guide target (checked by a test). */
export function unknownTargets(): string[] {
  return TOURS.flatMap((t) => t.steps.filter((s) => s.target && !guideTarget(s.target)).map((s) => `${t.id}:${s.target}`));
}

/**
 * "What can I do here?": the guide targets that belong to `view` (or are
 * everywhere and relevant), limited to those `present` on screen.
 */
export function thingsHere(view: SectionId, present: (id: string) => boolean): { id: string; what: string }[] {
  return GUIDE_TARGETS.filter((t) => t.view === view && present(t.id)).map((t) => ({ id: t.id, what: t.what }));
}
