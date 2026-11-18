/**
 * Nightgrid's UI sounds (D-178): original synthetic blips, glitch chirps and
 * data textures, made from the recipes below (see synth.ts). No sound files
 * or third-party packs. Never used for terminal output or live numbers.
 */
import type { Recipe } from "./synth";

export type SoundCategory = "alerts" | "notices" | "actions" | "dialogs" | "drift" | "quests" | "tasks" | "navigation";

export const CATEGORIES: { id: SoundCategory; label: string; hint: string }[] = [
  { id: "alerts", label: "Alerts", hint: "An alert fires (warning or critical) or clears." },
  { id: "notices", label: "Inbox", hint: "A new notice lands in the inbox." },
  { id: "actions", label: "Actions", hint: "Something you started worked or failed: container actions, applies, power." },
  { id: "dialogs", label: "Dialogs", hint: "A confirm dialog opens." },
  { id: "drift", label: "DRIFT", hint: "DRIFT finished a reply." },
  { id: "quests", label: "Quests", hint: "A guide's step is done, or the whole quest." },
  { id: "tasks", label: "Long tasks", hint: "A scan, test restore, drill or speed test finished." },
  { id: "navigation", label: "View switch", hint: "A soft tick when you change views. Off by default." },
];

export type SoundId =
  | "alertCritical"
  | "alertWarn"
  | "alertClear"
  | "notice"
  | "ok"
  | "fail"
  | "dialog"
  | "dialogDanger"
  | "driftReply"
  | "questStep"
  | "questComplete"
  | "taskDone"
  | "taskFail"
  | "tick";

export interface Sound {
  id: SoundId;
  category: SoundCategory;
  label: string;
  recipe: Recipe;
}

const semis = (base: number, n: number) => base * Math.pow(2, n / 12);

export const SOUNDS: Sound[] = [
  {
    id: "alertCritical",
    category: "alerts",
    label: "Critical alert",
    recipe: {
      level: 0.8,
      voices: [
        // Three hard, detuned saw pulses with a downward stepped bend.
        ...[0, 0.16, 0.32].flatMap((at) => [
          { wave: "saw" as const, at, dur: 0.12, f0: 660, f1: 520, steps: 4, gain: 0.5, lp: 3200, decay: 0.08 },
          { wave: "square" as const, at, dur: 0.12, f0: 666, f1: 524, steps: 4, gain: 0.25, lp: 2400, decay: 0.08, width: 0.3 },
        ]),
        { wave: "noise", at: 0, dur: 0.03, f0: 1, gain: 0.35, hp: 3000, decay: 0.01 },
        { wave: "sine", at: 0, dur: 0.5, f0: 110, gain: 0.25, decay: 0.2 },
      ],
    },
  },
  {
    id: "alertWarn",
    category: "alerts",
    label: "Warning alert",
    recipe: {
      level: 0.6,
      voices: [
        { wave: "tri", at: 0, dur: 0.09, f0: 740, gain: 0.6, decay: 0.05 },
        { wave: "tri", at: 0.12, dur: 0.14, f0: 587, gain: 0.6, decay: 0.07 },
        { wave: "square", at: 0, dur: 0.26, f0: 1480, gain: 0.08, crush: 6, gate: [40, 0.5], decay: 0.1, lp: 4000 },
      ],
    },
  },
  {
    id: "alertClear",
    category: "alerts",
    label: "Alert cleared",
    recipe: {
      level: 0.5,
      echo: [0.09, 0.3, 0.3],
      voices: [
        { wave: "sine", at: 0, dur: 0.18, f0: 784, f1: 1175, steps: 3, gain: 0.6, decay: 0.08 },
        { wave: "tri", at: 0.05, dur: 0.2, f0: 1568, gain: 0.25, decay: 0.07 },
      ],
    },
  },
  {
    id: "notice",
    category: "notices",
    label: "Inbox notice",
    recipe: {
      level: 0.5,
      echo: [0.07, 0.25, 0.25],
      voices: [
        // A quick data chirp: three stepped sine blips over a hiss of static.
        { wave: "sine", at: 0, dur: 0.05, f0: 1320, gain: 0.6, decay: 0.03 },
        { wave: "sine", at: 0.055, dur: 0.05, f0: 1760, gain: 0.5, decay: 0.03 },
        { wave: "sine", at: 0.11, dur: 0.09, f0: 2217, gain: 0.45, decay: 0.05 },
        { wave: "noise", at: 0, dur: 0.16, f0: 1, gain: 0.08, hp: 4000, lp: 9000, gate: [60, 0.4], decay: 0.06 },
      ],
    },
  },
  {
    id: "ok",
    category: "actions",
    label: "Action done",
    recipe: {
      level: 0.45,
      voices: [
        { wave: "square", at: 0, dur: 0.05, f0: 988, gain: 0.4, lp: 3500, decay: 0.03, width: 0.25 },
        { wave: "square", at: 0.055, dur: 0.08, f0: 1480, gain: 0.4, lp: 3500, decay: 0.04, width: 0.25 },
      ],
    },
  },
  {
    id: "fail",
    category: "actions",
    label: "Action failed",
    recipe: {
      level: 0.55,
      voices: [
        // A glitchy, stuttered low buzz falling away.
        { wave: "saw", at: 0, dur: 0.24, f0: 220, f1: 130, gain: 0.6, lp: 1600, crush: 8, gate: [28, 0.6], decay: 0.12 },
        { wave: "square", at: 0, dur: 0.24, f0: 223, f1: 128, gain: 0.3, lp: 1200, decay: 0.12 },
        { wave: "noise", at: 0, dur: 0.05, f0: 1, gain: 0.25, hp: 2000, crush: 4, decay: 0.02 },
      ],
    },
  },
  {
    id: "dialog",
    category: "dialogs",
    label: "Confirm dialog",
    recipe: {
      level: 0.35,
      voices: [
        // A filtered sweep up, like a panel powering on.
        { wave: "square", at: 0, dur: 0.12, f0: 220, f1: 660, gain: 0.5, lp: 1800, decay: 0.07, width: 0.35 },
        { wave: "sine", at: 0.08, dur: 0.06, f0: 1320, gain: 0.3, decay: 0.03 },
      ],
    },
  },
  {
    id: "dialogDanger",
    category: "dialogs",
    label: "Dangerous confirm",
    recipe: {
      level: 0.45,
      voices: [
        { wave: "saw", at: 0, dur: 0.18, f0: 180, f1: 330, gain: 0.5, lp: 1400, decay: 0.1 },
        { wave: "saw", at: 0, dur: 0.18, f0: 191, f1: 350, gain: 0.35, lp: 1400, decay: 0.1 },
        { wave: "square", at: 0.13, dur: 0.07, f0: 466, gain: 0.25, lp: 2200, crush: 4, decay: 0.04 },
      ],
    },
  },
  {
    id: "driftReply",
    category: "drift",
    label: "DRIFT reply",
    recipe: {
      level: 0.4,
      voices: [
        // A soft data-stream texture: a burble of random-sounding blips (fixed
        // pitches so every play is the same), resolving on a clear tone.
        ...[1568, 2093, 1760, 2349, 1976, 2637].map((f, i) => ({ wave: "sine" as const, at: i * 0.028, dur: 0.025, f0: f, gain: 0.35, decay: 0.012 })),
        { wave: "noise", at: 0, dur: 0.17, f0: 1, gain: 0.06, hp: 5000, gate: [90, 0.35], decay: 0.08 },
        { wave: "tri", at: 0.17, dur: 0.16, f0: semis(1175, 0), gain: 0.45, decay: 0.07, vib: [9, 0.15] },
      ],
    },
  },
  {
    id: "questStep",
    category: "quests",
    label: "Quest updated",
    recipe: {
      level: 0.5,
      echo: [0.08, 0.3, 0.3],
      voices: [
        { wave: "square", at: 0, dur: 0.1, f0: 660, f1: 1320, steps: 5, gain: 0.35, lp: 4000, width: 0.25, decay: 0.07 },
        { wave: "noise", at: 0, dur: 0.04, f0: 1, gain: 0.12, hp: 5000, crush: 3, decay: 0.015 },
        { wave: "sine", at: 0.1, dur: 0.14, f0: 1760, gain: 0.35, decay: 0.06 },
      ],
    },
  },
  {
    id: "questComplete",
    category: "quests",
    label: "Quest complete",
    recipe: {
      level: 0.6,
      echo: [0.11, 0.35, 0.3],
      voices: [
        // A rising minor arpeggio landing on a held, shimmering octave.
        ...[0, 3, 7, 12].map((n, i) => ({ wave: "square" as const, at: i * 0.075, dur: 0.07, f0: semis(440, n), gain: 0.35, lp: 3800, width: 0.3, decay: 0.05 })),
        { wave: "tri", at: 0.3, dur: 0.45, f0: 880, gain: 0.45, vib: [6, 0.12], decay: 0.18 },
        { wave: "sine", at: 0.3, dur: 0.45, f0: 1760, gain: 0.18, decay: 0.15 },
        { wave: "noise", at: 0.28, dur: 0.12, f0: 1, gain: 0.08, hp: 6000, gate: [50, 0.5], decay: 0.05 },
      ],
    },
  },
  {
    id: "taskDone",
    category: "tasks",
    label: "Task finished",
    recipe: {
      level: 0.45,
      voices: [
        // A scanner sweep that locks on.
        { wave: "sine", at: 0, dur: 0.16, f0: 400, f1: 1600, gain: 0.35, decay: 0.12, vib: [30, 0.3] },
        { wave: "square", at: 0.17, dur: 0.05, f0: 1568, gain: 0.3, lp: 3500, width: 0.25, decay: 0.03 },
        { wave: "square", at: 0.23, dur: 0.07, f0: 1568, gain: 0.3, lp: 3500, width: 0.25, decay: 0.04 },
      ],
    },
  },
  {
    id: "taskFail",
    category: "tasks",
    label: "Task failed",
    recipe: {
      level: 0.5,
      voices: [
        { wave: "sine", at: 0, dur: 0.16, f0: 1200, f1: 300, gain: 0.35, decay: 0.12, vib: [30, 0.3] },
        { wave: "saw", at: 0.17, dur: 0.16, f0: 196, gain: 0.45, lp: 1300, crush: 6, gate: [24, 0.55], decay: 0.08 },
      ],
    },
  },
  {
    id: "tick",
    category: "navigation",
    label: "View switch",
    recipe: {
      level: 0.22,
      voices: [
        { wave: "square", at: 0, dur: 0.018, f0: 2400, gain: 0.5, hp: 1200, lp: 6000, width: 0.2, decay: 0.006 },
        { wave: "noise", at: 0, dur: 0.012, f0: 1, gain: 0.15, hp: 6000, decay: 0.004 },
      ],
    },
  },
];

export const soundById = (id: SoundId): Sound => SOUNDS.find((s) => s.id === id)!;

/** Sound file names for exports: `alert-critical.wav`. */
export const fileName = (id: SoundId) => `${id.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}.wav`;
