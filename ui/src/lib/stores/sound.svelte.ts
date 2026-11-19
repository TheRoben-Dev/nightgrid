/**
 * UI sounds (D-178): plays the synthesized sounds in sound/sounds.ts. The
 * audio context is made on the first sound, and buffers are rendered once
 * per sound. Turned off by the `sound` feature; never used for terminal
 * output or live numbers.
 */
import { allowed, gainOf, type Last } from "../sound/gate";
import { render } from "../sound/synth";
import { soundById, type SoundId } from "../sound/sounds";
import { settings } from "./settings.svelte";

class SoundStore {
  private ctx: AudioContext | null = null;
  private buffers = new Map<SoundId, AudioBuffer>();
  private last: Last | null = null;
  private lastById = new Map<SoundId, number>();
  private unlocked = false;

  private context(): AudioContext | null {
    if (this.ctx) return this.ctx;
    const Ctor = typeof window !== "undefined" ? (window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext) : undefined;
    if (!Ctor) return null;
    try {
      this.ctx = new Ctor();
    } catch {
      return null;
    }
    return this.ctx;
  }

  /** Webviews start audio suspended until the user interacts; resume on the first click or key. */
  unlockOnGesture() {
    if (this.unlocked || typeof window === "undefined") return;
    this.unlocked = true;
    const resume = () => {
      if (this.ctx?.state === "suspended") void this.ctx.resume();
    };
    window.addEventListener("pointerdown", resume, { capture: true });
    window.addEventListener("keydown", resume, { capture: true });
  }

  private buffer(ctx: AudioContext, id: SoundId): AudioBuffer {
    let b = this.buffers.get(id);
    if (!b) {
      const data = render(soundById(id).recipe, ctx.sampleRate);
      b = ctx.createBuffer(1, data.length, ctx.sampleRate);
      b.copyToChannel(data, 0);
      this.buffers.set(id, b);
    }
    return b;
  }

  /** Plays a sound if the settings allow it now. */
  play(id: SoundId) {
    const s = settings.value.sound;
    const category = soundById(id).category;
    const now = new Date();
    const ok = allowed(
      { on: settings.on("sound"), ...s },
      id,
      category,
      { now, focused: typeof document === "undefined" || document.hasFocus(), last: this.last, lastSame: this.lastById.get(id) ?? null },
    );
    if (!ok) return;
    this.last = { id, category, at: now.getTime() };
    this.lastById.set(id, now.getTime());
    this.out(id, s.volume);
  }

  /** The test buttons in Settings: plays at the set volume whatever else is set. */
  test(id: SoundId) {
    this.out(id, Math.max(settings.value.sound.volume, 5));
  }

  private out(id: SoundId, volume: number) {
    const ctx = this.context();
    if (!ctx) return;
    try {
      if (ctx.state === "suspended") void ctx.resume();
      const src = ctx.createBufferSource();
      src.buffer = this.buffer(ctx, id);
      const g = ctx.createGain();
      g.gain.value = gainOf(volume);
      src.connect(g).connect(ctx.destination);
      src.start();
    } catch {
      // Sound is a nicety; never let it break what triggered it.
    }
  }
}

export const sound = new SoundStore();
