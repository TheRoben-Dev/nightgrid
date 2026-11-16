/**
 * Cron helper (M10p), both ways: a cron expression in plain words with its
 * next run times, and plain words ("weekdays at 6pm") as a cron expression
 * with the matching systemd OnCalendar line. Exact, so DRIFT doesn't have to
 * do the arithmetic.
 */

const DOW = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MON = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DOW_ABBR = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const MON_ABBR = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

const MACROS: Record<string, string> = {
  "@yearly": "0 0 1 1 *",
  "@annually": "0 0 1 1 *",
  "@monthly": "0 0 1 * *",
  "@weekly": "0 0 * * 0",
  "@daily": "0 0 * * *",
  "@midnight": "0 0 * * *",
  "@hourly": "0 * * * *",
};

interface Field {
  values: number[];
  any: boolean;
  step?: number;
}

export interface Cron {
  minute: Field;
  hour: Field;
  dom: Field;
  month: Field;
  dow: Field;
}

function field(raw: string, min: number, max: number, names?: string[]): Field | null {
  const vals = new Set<number>();
  let step: number | undefined;
  const name = (s: string) => {
    if (names) {
      const i = names.indexOf(s.toLowerCase().slice(0, 3));
      if (i >= 0) return i + min;
    }
    return /^\d+$/.test(s) ? Number(s) : NaN;
  };
  for (const part of raw.split(",")) {
    const [range, st] = part.split("/");
    const s = st === undefined ? 1 : Number(st);
    if (!Number.isInteger(s) || s < 1) return null;
    let lo: number;
    let hi: number;
    if (range === "*") {
      lo = min;
      hi = max;
      if (st !== undefined && raw.split(",").length === 1) step = s;
    } else if (range.includes("-")) {
      const [a, b] = range.split("-");
      lo = name(a);
      hi = name(b);
    } else {
      lo = name(range);
      hi = st === undefined ? lo : max;
    }
    if (max === 7 && hi === 7 && lo <= 7) hi = 7;
    if (!Number.isInteger(lo) || !Number.isInteger(hi) || lo < min || hi > max || lo > hi) return null;
    for (let v = lo; v <= hi; v += s) vals.add(v);
  }
  return { values: [...vals].sort((a, b) => a - b), any: raw === "*", step };
}

/** Parses a five-field expression (or @daily and friends). `@reboot` and bad input give null. */
export function parseCron(expr: string): Cron | null {
  const e = MACROS[expr.trim().toLowerCase()] ?? expr.trim();
  const p = e.split(/\s+/);
  if (p.length !== 5) return null;
  const minute = field(p[0], 0, 59);
  const hour = field(p[1], 0, 23);
  const dom = field(p[2], 1, 31);
  const month = field(p[3], 1, 12, MON_ABBR);
  const dow = field(p[4], 0, 7, DOW_ABBR);
  if (!minute || !hour || !dom || !month || !dow) return null;
  // 7 is Sunday too.
  dow.values = [...new Set(dow.values.map((v) => v % 7))].sort((a, b) => a - b);
  return { minute, hour, dom, month, dow };
}

const pad = (n: number) => String(n).padStart(2, "0");

function list(names: string[]): string {
  if (names.length <= 2) return names.join(" and ");
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
}

/** Runs of consecutive values become "Monday to Friday". */
function spans(vals: number[], label: (v: number) => string): string {
  const out: string[] = [];
  let i = 0;
  while (i < vals.length) {
    let j = i;
    while (j + 1 < vals.length && vals[j + 1] === vals[j] + 1) j++;
    out.push(j - i >= 2 ? `${label(vals[i])} to ${label(vals[j])}` : j > i ? `${label(vals[i])}, ${label(vals[j])}` : label(vals[i]));
    i = j + 1;
  }
  return list(out.flatMap((s) => (s.includes(" to ") ? [s] : s.split(", "))));
}

const ordinal = (n: number) => `${n}${n % 10 === 1 && n !== 11 ? "st" : n % 10 === 2 && n !== 12 ? "nd" : n % 10 === 3 && n !== 13 ? "rd" : "th"}`;

/** The expression in plain words ("At 03:30, Monday to Friday"). */
export function describeCron(expr: string): string | null {
  if (expr.trim().toLowerCase() === "@reboot") return "Once, every time the machine starts.";
  const c = parseCron(expr);
  if (!c) return null;
  let time: string;
  const m = c.minute;
  const h = c.hour;
  if (m.any && h.any) time = "Every minute";
  else if (m.step && h.any) time = `Every ${m.step} minutes`;
  else if (m.step && !h.any) time = `Every ${m.step} minutes during hour${h.values.length > 1 ? "s" : ""} ${spans(h.values, pad)}`;
  else if (m.any) time = `Every minute during hour${h.values.length > 1 ? "s" : ""} ${spans(h.values, pad)}`;
  else if (h.any) time = m.values.length === 1 ? (m.values[0] === 0 ? "Every hour, on the hour" : `Every hour at minute ${m.values[0]}`) : `Every hour at minutes ${list(m.values.map(String))}`;
  else if (h.step && m.values.length === 1) time = `Every ${h.step} hours at minute ${m.values[0]}${h.values[0] ? ` from ${pad(h.values[0])}:00` : ""}`;
  else if (h.values.length * m.values.length <= 6) time = `At ${list(h.values.flatMap((hh) => m.values.map((mm) => `${pad(hh)}:${pad(mm)}`)))}`;
  else time = `At minute ${list(m.values.map(String))} of hours ${spans(h.values, pad)}`;

  const parts: string[] = [];
  const domR = !c.dom.any;
  const dowR = !c.dow.any;
  const days = dowR ? (c.dow.values.length === 7 ? "every day" : spans(c.dow.values, (v) => DOW[v])) : "";
  const doms = domR ? (c.dom.step && c.dom.values[0] === 1 ? `every ${c.dom.step} days of the month` : `on the ${list(c.dom.values.map(ordinal))} of the month`) : "";
  if (domR && dowR) parts.push(`${doms} and also every ${days}`);
  else if (domR) parts.push(doms);
  else if (dowR) parts.push(days === "every day" ? days : `on ${days}`);
  if (!c.month.any) parts.push(`in ${spans(c.month.values, (v) => MON[v - 1])}`);
  return `${time}${parts.length ? `, ${parts.join(", ")}` : ""}.`;
}

function matchesDay(c: Cron, d: Date): boolean {
  const dom = c.dom.values.includes(d.getDate());
  const dow = c.dow.values.includes(d.getDay());
  if (!c.dom.any && !c.dow.any) return dom || dow;
  return dom && dow;
}

/** The next `n` run times after `from`, in this computer's time zone. */
export function nextRuns(expr: string, from: Date, n = 3): Date[] {
  const c = parseCron(expr);
  if (!c) return [];
  const out: Date[] = [];
  const d = new Date(from.getTime());
  d.setSeconds(0, 0);
  d.setMinutes(d.getMinutes() + 1);
  const limit = from.getTime() + 5 * 366 * 86400_000;
  while (out.length < n && d.getTime() < limit) {
    if (!c.month.values.includes(d.getMonth() + 1)) {
      d.setMonth(d.getMonth() + 1, 1);
      d.setHours(0, 0);
      continue;
    }
    if (!matchesDay(c, d)) {
      d.setDate(d.getDate() + 1);
      d.setHours(0, 0);
      continue;
    }
    if (!c.hour.values.includes(d.getHours())) {
      d.setHours(d.getHours() + 1, 0);
      continue;
    }
    if (!c.minute.values.includes(d.getMinutes())) {
      d.setMinutes(d.getMinutes() + 1);
      continue;
    }
    out.push(new Date(d.getTime()));
    d.setMinutes(d.getMinutes() + 1);
  }
  return out;
}

/** The systemd OnCalendar= equivalent, for the shapes that translate cleanly. */
export function toOnCalendar(expr: string): string | null {
  const c = parseCron(expr);
  if (!c) return null;
  const f = (x: Field, w = 2, min = 0) => (x.any ? "*" : x.step && x.values[0] === min ? `${pad(min)}/${x.step}` : x.values.map((v) => String(v).padStart(w, "0")).join(","));
  if (!c.dom.any && !c.dow.any) return null;
  const dow = c.dow.any ? "" : `${spans(c.dow.values, (v) => DOW[v].slice(0, 3)).replace(/ to /g, "..").replace(/ and |, /g, ",")} `;
  return `${dow}*-${f(c.month, 2, 1)}-${f(c.dom, 2, 1)} ${f(c.hour)}:${f(c.minute)}:00`;
}

/** "3", "3am", "3:30 pm", "15:45", "noon", "midnight" → [hour, minute]. */
function parseTime(text: string): [number, number] | null {
  const t = text.toLowerCase();
  if (/\bmidnight\b/.test(t)) return [0, 0];
  if (/\bnoon\b|\bmidday\b/.test(t)) return [12, 0];
  const m = /\b(?:at|@)\s*(\d{1,2})(?:[:.h](\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?/.exec(t) ?? /\b(\d{1,2})(?:[:.](\d{2}))\s*(am|pm)?\b/.exec(t) ?? /\b(\d{1,2})\s*(am|pm)\b/.exec(t);
  if (!m) return null;
  let h = Number(m[1]);
  const min = m[2] ? Number(m[2]) : 0;
  const ap = m[3]?.replace(/\./g, "");
  if (ap === "pm" && h < 12) h += 12;
  if (ap === "am" && h === 12) h = 0;
  if (h > 23 || min > 59) return null;
  return [h, min];
}

const DAY_WORDS: [RegExp, number][] = [
  [/\bsun(day)?s?\b/, 0],
  [/\bmon(day)?s?\b/, 1],
  [/\btue(s|sday)?s?\b/, 2],
  [/\bwed(nesday)?s?\b/, 3],
  [/\bthu(r|rs|rsday)?s?\b/, 4],
  [/\bfri(day)?s?\b/, 5],
  [/\bsat(urday)?s?\b/, 6],
];

export interface CronDraft {
  expr: string;
  note?: string;
}

/** Plain words → a cron expression, for the common shapes. Null when it can't tell. */
export function cronFromWords(text: string): CronDraft | null {
  const t = ` ${text.toLowerCase().replace(/[,]/g, " ")} `;
  if (/\b(at|on) (re)?boot\b|\bat startup\b|\bon start\b/.test(t)) return { expr: "@reboot" };
  const every = /\bevery\s+(\d+)\s*(minute|min|hour|hr|day)s?\b/.exec(t);
  if (every) {
    const n = Number(every[1]);
    const unit = every[2];
    if (unit.startsWith("min")) {
      if (n < 1 || n > 59) return null;
      return { expr: `*/${n} * * * *`, note: 60 % n ? `${n} doesn't divide 60, so the gap is shorter once an hour (cron restarts counting at minute 0).` : undefined };
    }
    if (unit.startsWith("h")) {
      if (n < 1 || n > 23) return null;
      return { expr: `0 */${n} * * *`, note: 24 % n ? `${n} doesn't divide 24, so the last gap of the day is shorter.` : undefined };
    }
    const tm = parseTime(t) ?? [0, 0];
    return { expr: `${tm[1]} ${tm[0]} */${n} * *`, note: "Cron counts days of the month, so the gap resets on the 1st (a systemd timer with OnUnitActiveSec= keeps an exact interval)." };
  }
  if (/\bevery minute\b/.test(t)) return { expr: "* * * * *" };
  if (/\b(hourly|every hour|each hour)\b/.test(t)) return { expr: "0 * * * *" };
  const time = parseTime(t);
  const [h, m] = time ?? [0, 0];
  if (/\b(yearly|annually|every year|once a year)\b/.test(t)) return { expr: `${m} ${h} 1 1 *` };
  const domM = /\b(?:on the |the )?(\d{1,2})(?:st|nd|rd|th)\b(?: (?:day )?of (?:the|each|every) month)?/.exec(t);
  if (/\b(monthly|every month|each month|of the month|of each month|of every month)\b/.test(t) || (domM && /\bmonth\b/.test(t))) {
    const first = /\bfirst day\b/.test(t) ? 1 : /\blast day\b/.test(t) ? null : domM ? Number(domM[1]) : 1;
    if (first === null) return { expr: `${m} ${h} 28-31 * *`, note: "Cron has no \"last day of the month\": this runs on the 28th to 31st; add `[ \"$(date -d tomorrow +%d)\" = 01 ] &&` before the command to keep only the last one." };
    if (first < 1 || first > 31) return null;
    return { expr: `${m} ${h} ${first} * *`, note: first > 28 ? `Months without a ${first}th day are skipped.` : undefined };
  }
  let dow: number[];
  if (/\bweekdays?\b|\bwork ?days?\b|\bmonday (to|through|-) friday\b|\bmon-fri\b/.test(t)) dow = [1, 2, 3, 4, 5];
  else if (/\bweekends?\b/.test(t)) dow = [0, 6];
  else dow = DAY_WORDS.filter(([re]) => re.test(t)).map(([, d]) => d);
  if (dow.length) {
    const d = dow.length === 5 && dow.join() === "1,2,3,4,5" ? "1-5" : dow.join(",");
    return { expr: `${m} ${h} * * ${d}`, note: time ? undefined : "No time given: midnight." };
  }
  if (/\b(daily|every day|each day|nightly|every night|every morning|every evening|once a day)\b/.test(t) || time) {
    let hh = h;
    if (!time && /\bmorning\b/.test(t)) hh = 6;
    if (!time && /\bevening\b/.test(t)) hh = 18;
    if (!time && /\bnight(ly)?\b/.test(t)) hh = 2;
    return { expr: `${m} ${hh} * * *`, note: time ? undefined : `No time given: ${pad(hh)}:00.` };
  }
  return null;
}

const CRON_IN_TEXT = /(^|[\s`'"])((?:@(?:reboot|yearly|annually|monthly|weekly|daily|midnight|hourly))|(?:(?:[\d*][\d*,/-]*)\s+(?:[\d*][\d*,/-]*)\s+(?:[\d*?][\d*,/-]*)\s+(?:[\d*a-z][\d*,/a-z-]*)\s+(?:[\d*a-z][\d*,/a-z-]*)))(?=$|[\s`'"])/im;

/** A cron expression written in the text, if any (it must parse). */
export function findCron(text: string): string | null {
  const m = CRON_IN_TEXT.exec(text);
  if (!m) return null;
  const e = m[2].trim();
  return e.toLowerCase() === "@reboot" || parseCron(e) ? e : null;
}
