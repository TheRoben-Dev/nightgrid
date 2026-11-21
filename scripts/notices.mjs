#!/usr/bin/env node
// Transmissions (D-202): edits and builds the developer notice feed.
//
//   node scripts/notices.mjs check              validate docs/notices/notices.json
//   node scripts/notices.mjs add                add a notice from NOTICE_* env vars (see below)
//   node scripts/notices.mjs withdraw <id>      remove a notice (apps hide it on their next check)
//   node scripts/notices.mjs build <out-dir>    write <out-dir>/notices.json for signing
//
// `add` reads NOTICE_ID (optional: made from the title), NOTICE_LEVEL
// (info|warn|critical), NOTICE_TITLE, NOTICE_BODY (markdown; a literal \n is
// a line break), NOTICE_LINK_LABEL + NOTICE_LINK_TO (kb:shelf/slug,
// view:id[/tab] or https://…), NOTICE_OS (comma list of macos,windows,linux),
// NOTICE_MIN_APP, NOTICE_MAX_APP, NOTICE_FEATURES (comma list),
// NOTICE_STARTS, NOTICE_EXPIRES (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ).
//
// The rules match core/src/broadcast.rs, which also checks the committed
// file in CI (`the_repository_feed_is_valid`).
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

export const SOURCE = "docs/notices/notices.json";
export const FORMAT = 1;
const OSES = ["macos", "windows", "linux"];
const LEVELS = ["info", "warn", "critical"];
const FIELDS = ["id", "level", "title", "body", "link", "os", "minApp", "maxApp", "features", "starts", "expires"];

const ID = /^[a-z0-9][a-z0-9-]{0,63}$/;
const SEMVER = /^v?\d+(\.\d+){0,2}([-+].*)?$/;

export function parseTime(s) {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}):(\d{2})Z)?$/.exec(s);
  if (!m) return null;
  const [y, mo, d, h = 0, mi = 0, se = 0] = m.slice(1).map((x) => (x === undefined ? 0 : Number(x)));
  if (mo < 1 || mo > 12 || h > 23 || mi > 59 || se > 59) return null;
  const t = Date.UTC(y, mo - 1, d, h, mi, se);
  if (new Date(t).getUTCDate() !== d) return null;
  return t / 1000;
}

export function linkOk(to) {
  if (to.startsWith("kb:")) return /^kb:[a-z]+\/[A-Za-z0-9_-]+$/.test(to);
  if (to.startsWith("view:")) return /^view:[a-z]+(\/[a-z-]+)?$/.test(to);
  if (to.startsWith("https://")) {
    const host = to.slice(8).split(/[/?#]/)[0];
    return host.length > 0 && !host.includes("@") && to.length <= 500 && !/[\s\p{Cc}]/u.test(to);
  }
  return false;
}

const chars = (s) => [...s].length;

/** Returns a list of problems (empty when the notices are fine). */
export function problems(notices) {
  if (!Array.isArray(notices)) return ["`notices` must be a list"];
  const out = [];
  if (notices.length > 50) out.push("at most 50 notices");
  const seen = new Set();
  for (const n of notices) {
    const id = n?.id ?? "?";
    const bad = (m) => out.push(`${id}: ${m}`);
    for (const k of Object.keys(n ?? {})) if (!FIELDS.includes(k)) bad(`unknown field "${k}"`);
    if (typeof n.id !== "string" || !ID.test(n.id)) bad("id must be 1–64 lowercase letters, digits and dashes");
    if (seen.has(n.id)) bad("id is used twice");
    seen.add(n.id);
    if (!LEVELS.includes(n.level)) bad("level must be info, warn or critical");
    if (typeof n.title !== "string" || !n.title.trim() || chars(n.title.trim()) > 100) bad("title must be 1–100 characters");
    if (typeof n.body !== "string" || !n.body.trim() || chars(n.body) > 4000) bad("body must be 1–4000 characters");
    if (n.link != null) {
      const l = n.link;
      if (typeof l.label !== "string" || !l.label.trim() || chars(l.label.trim()) > 40) bad("link label must be 1–40 characters");
      if (typeof l.to !== "string" || !linkOk(l.to)) bad("link must be kb:shelf/slug, view:id[/tab] or https://…");
    }
    for (const o of n.os ?? []) if (!OSES.includes(o)) bad(`unknown os "${o}"`);
    for (const k of ["minApp", "maxApp"]) if (n[k] != null && !SEMVER.test(String(n[k]))) bad(`${k} "${n[k]}" isn't a version like 1.22.0`);
    for (const f of n.features ?? []) if (!ID.test(f)) bad(`"${f}" isn't a feature id`);
    const t = {};
    for (const k of ["starts", "expires"]) {
      if (n[k] == null) continue;
      t[k] = parseTime(String(n[k]));
      if (t[k] === null) bad(`${k} "${n[k]}" isn't YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ`);
    }
    if (t.starts != null && t.expires != null && t.expires <= t.starts) bad("it expires before it starts");
  }
  return out;
}

const list = (s) =>
  (s ?? "")
    .split(",")
    .map((x) => x.trim().toLowerCase())
    .filter(Boolean);

/** Builds a notice from NOTICE_* variables (empty ones are left out). */
export function fromEnv(env) {
  const v = (k) => (env[`NOTICE_${k}`] ?? "").trim();
  const title = v("TITLE");
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40)
    .replace(/-+$/, "");
  const date = new Date().toISOString().slice(0, 10);
  const n = {
    id: v("ID") || `${date}-${slug || "notice"}`,
    level: v("LEVEL") || "info",
    title,
    body: v("BODY").replace(/\\n/g, "\n"),
  };
  if (v("LINK_TO")) n.link = { label: v("LINK_LABEL") || "Open", to: v("LINK_TO") };
  if (list(v("OS")).length) n.os = list(v("OS"));
  if (v("MIN_APP")) n.minApp = v("MIN_APP");
  if (v("MAX_APP")) n.maxApp = v("MAX_APP");
  if (list(v("FEATURES")).length) n.features = list(v("FEATURES"));
  if (v("STARTS")) n.starts = v("STARTS");
  if (v("EXPIRES")) n.expires = v("EXPIRES");
  return n;
}

const read = () => JSON.parse(readFileSync(SOURCE, "utf8"));
const write = (src) => writeFileSync(SOURCE, `${JSON.stringify(src, null, 2)}\n`);
const fail = (msgs) => {
  for (const m of msgs) console.error(`::error::${m}`);
  process.exit(1);
};

function main([cmd, arg]) {
  const src = read();
  const errs = problems(src.notices);
  if (cmd === "check") {
    if (errs.length) fail(errs);
    console.log(`${src.notices.length} notice(s) OK`);
  } else if (cmd === "add") {
    if (errs.length) fail(errs);
    const n = fromEnv(process.env);
    if (src.notices.some((x) => x.id === n.id)) fail([`a notice with id ${n.id} already exists`]);
    const next = [...src.notices, n];
    const e = problems(next);
    if (e.length) fail(e);
    write({ ...src, notices: next });
    console.log(`added ${n.id}`);
  } else if (cmd === "withdraw") {
    if (!src.notices.some((x) => x.id === arg)) fail([`no notice with id ${arg}`]);
    write({ ...src, notices: src.notices.filter((x) => x.id !== arg) });
    console.log(`withdrew ${arg}`);
  } else if (cmd === "build") {
    if (errs.length) fail(errs);
    mkdirSync(arg, { recursive: true });
    const feed = { format: FORMAT, version: Math.floor(Date.now() / 1000), notices: src.notices };
    writeFileSync(join(arg, "notices.json"), JSON.stringify(feed));
    console.log(`built feed v${feed.version} with ${feed.notices.length} notice(s)`);
  } else {
    fail(["usage: notices.mjs check | add | withdraw <id> | build <out-dir>"]);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) main(process.argv.slice(2));
