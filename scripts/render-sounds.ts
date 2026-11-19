// Renders every UI sound to a WAV plus INDEX.md for review (D-178):
//   npx tsx scripts/render-sounds.ts <out-dir>
import { writeFileSync, mkdirSync } from "node:fs";
import { render, wav, duration } from "../ui/src/lib/sound/synth";
import { SOUNDS, fileName, CATEGORIES } from "../ui/src/lib/sound/sounds";
const dir = process.argv[2];
mkdirSync(dir, { recursive: true });
let idx = "# Nightgrid UI sounds \n\nAll synthesized from recipes in ui/src/lib/sound/sounds.ts. 16-bit mono WAV, 44.1 kHz. Levels shown at full volume (the app default is 40%).\n\n| File | Sound | Plays when | Length |\n|---|---|---|---|\n";
const when: Record<string,string> = {
  alertCritical: "A critical alert fires (host offline, pool degraded...)",
  alertWarn: "A warning alert fires",
  alertClear: "An alert clears",
  notice: "A new inbox notice (not alerts, they have their own)",
  ok: "An action you started succeeded (container start/stop, apply, power, package run)",
  fail: "An action you started failed",
  dialog: "A confirm dialog opens",
  dialogDanger: "A red (destructive) confirm dialog opens",
  driftReply: "DRIFT finishes a reply",
  questStep: "A quest step gets ticked",
  questComplete: "Every step of a quest is done",
  taskDone: "A network scan, test restore, drill or speed test finished OK",
  taskFail: "A long task finished with a failure",
  tick: "View switch (off by default)",
};
for (const s of SOUNDS) {
  const f = fileName(s.id);
  writeFileSync(`${dir}/${f}`, wav(render(s.recipe)));
  idx += `| ${f} | ${s.label} | ${when[s.id]} | ${(duration(s.recipe)*1000).toFixed(0)} ms |\n`;
}
idx += "\nCategories (each can be muted in Settings → SOUND): " + CATEGORIES.map(c=>c.label).join(", ") + "\n";
writeFileSync(`${dir}/INDEX.md`, idx);
console.log("ok");
