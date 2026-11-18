/**
 * A tiny offline synth for UI sounds (D-178). Every sound is a recipe of
 * voices rendered sample by sample into a mono buffer, so the app plays the
 * exact sound a WAV export holds, with no audio files or packs to ship.
 * Pure and deterministic: the same recipe always gives the same samples.
 */

export type Wave = "sine" | "square" | "tri" | "saw" | "noise";

export interface Voice {
  wave: Wave;
  /** Start, seconds from the sound's start. */
  at: number;
  dur: number;
  /** Start and end frequency in Hz (glide between them). Ignored by noise unless `lp`/`hp` track it. */
  f0: number;
  f1?: number;
  /** Quantize the glide into this many pitch steps: a digital, stepped chirp. */
  steps?: number;
  /** Peak level, 0–1. */
  gain: number;
  /** Attack in seconds (default 3 ms). */
  attack?: number;
  /** Exponential decay time constant in seconds; default: dur / 4. */
  decay?: number;
  /** One-pole low-pass cutoff in Hz. */
  lp?: number;
  /** One-pole high-pass cutoff in Hz. */
  hp?: number;
  /** Sample-and-hold rate reduction (2 = half rate): lo-fi grit. */
  crush?: number;
  /** Vibrato: rate in Hz and depth in semitones. */
  vib?: [number, number];
  /** Stutter gate: rate in Hz and duty 0–1, for glitchy chops. */
  gate?: [number, number];
  /** Square pulse width, 0–1 (default 0.5). */
  width?: number;
}

export interface Recipe {
  voices: Voice[];
  /** Feedback echo: delay seconds, feedback 0–0.8, wet mix 0–1. */
  echo?: [number, number, number];
  /** Peak level after normalizing, 0–1. */
  level: number;
}

export const SAMPLE_RATE = 44100;

/** xorshift32: noise that is the same on every render. */
function rng(seed: number) {
  let x = seed | 0 || 1;
  return () => {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    return ((x >>> 0) / 0xffffffff) * 2 - 1;
  };
}

const TAU = Math.PI * 2;

function osc(wave: Wave, phase: number, width: number, noise: () => number): number {
  const p = phase - Math.floor(phase);
  switch (wave) {
    case "sine":
      return Math.sin(TAU * p);
    case "square":
      return p < width ? 1 : -1;
    case "tri":
      return 1 - 4 * Math.abs(p - 0.5);
    case "saw":
      return 2 * p - 1;
    case "noise":
      return noise();
  }
}

/** The length of a recipe in seconds, echo tail included. */
export function duration(r: Recipe): number {
  const end = Math.max(0, ...r.voices.map((v) => v.at + v.dur));
  if (!r.echo) return end;
  const [time, fb] = r.echo;
  // Repeats until the echo falls under -40 dB.
  const repeats = fb > 0 ? Math.min(12, Math.ceil(Math.log(0.01) / Math.log(fb))) : 1;
  return end + time * repeats;
}

export function render(r: Recipe, rate = SAMPLE_RATE, seed = 0x5eed): Float32Array<ArrayBuffer> {
  const n = Math.ceil(duration(r) * rate) + 1;
  const out = new Float32Array(n);
  r.voices.forEach((v, i) => {
    const noise = rng(seed + i * 7919);
    const start = Math.floor(v.at * rate);
    const len = Math.floor(v.dur * rate);
    const attack = Math.max(1, Math.floor((v.attack ?? 0.003) * rate));
    const decay = v.decay ?? v.dur / 4;
    const release = Math.min(len, Math.floor(0.004 * rate));
    const f1 = v.f1 ?? v.f0;
    const width = v.width ?? 0.5;
    const crush = Math.max(1, Math.floor(v.crush ?? 1));
    const lpA = v.lp ? 1 - Math.exp((-TAU * v.lp) / rate) : 1;
    const hpA = v.hp ? Math.exp((-TAU * v.hp) / rate) : 0;
    let phase = 0;
    let held = 0;
    let lp = 0;
    let hpPrevIn = 0;
    let hpOut = 0;
    for (let k = 0; k < len && start + k < n; k++) {
      const t = k / rate;
      let prog = len > 1 ? k / (len - 1) : 0;
      if (v.steps && v.steps > 1) prog = Math.floor(prog * v.steps) / (v.steps - 1);
      prog = Math.min(1, prog);
      // Exponential glide sounds even across octaves.
      let f = v.f0 * Math.pow(f1 / v.f0, prog);
      if (v.vib) f *= Math.pow(2, (v.vib[1] * Math.sin(TAU * v.vib[0] * t)) / 12);
      phase += f / rate;
      if (k % crush === 0) held = osc(v.wave, phase, width, noise);
      let s = held;
      if (v.lp) s = lp += lpA * (s - lp);
      if (v.hp) {
        hpOut = hpA * (hpOut + s - hpPrevIn);
        hpPrevIn = s;
        s = hpOut;
      }
      let env = k < attack ? k / attack : Math.exp(-(k - attack) / rate / decay);
      if (k > len - release) env *= (len - k) / release;
      if (v.gate && (t * v.gate[0]) % 1 > v.gate[1]) env = 0;
      out[start + k] += s * env * v.gain;
    }
  });
  if (r.echo) {
    const [time, fb, mix] = r.echo;
    const d = Math.max(1, Math.floor(time * rate));
    const wet = new Float32Array(n);
    for (let k = d; k < n; k++) wet[k] = (out[k - d] + wet[k - d] * fb) * (k - d >= 0 ? 1 : 0);
    for (let k = 0; k < n; k++) out[k] += wet[k] * mix;
  }
  let peak = 0;
  for (const s of out) peak = Math.max(peak, Math.abs(s));
  if (peak > 0) for (let k = 0; k < n; k++) out[k] *= r.level / peak;
  // 2 ms fade in and out so nothing clicks.
  const fade = Math.min(Math.floor(n / 2), Math.floor(0.002 * rate));
  for (let k = 0; k < fade; k++) {
    out[k] *= k / fade;
    out[n - 1 - k] *= k / fade;
  }
  return out;
}

/** 16-bit mono PCM WAV bytes. */
export function wav(samples: Float32Array<ArrayBuffer>, rate = SAMPLE_RATE): Uint8Array {
  const bytes = new Uint8Array(44 + samples.length * 2);
  const v = new DataView(bytes.buffer);
  const str = (o: number, s: string) => [...s].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  v.setUint32(4, 36 + samples.length * 2, true);
  str(8, "WAVE");
  str(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  str(36, "data");
  v.setUint32(40, samples.length * 2, true);
  samples.forEach((s, i) => v.setInt16(44 + i * 2, Math.round(Math.max(-1, Math.min(1, s)) * 32767), true));
  return bytes;
}
