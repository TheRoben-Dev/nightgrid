// Types for codenames.mjs, for the UI tests that check it (ui/tools/codenames.test.ts).
export const ADJECTIVES: string[];
export const NOUNS: string[];
export const DENY: string[];
export const HEADING_RE: RegExp;
export const UNRELEASED_RE: RegExp;
export const NO_NOTES: string;
export function denied(name: string): boolean;
export function pickCodename(version: string, used?: string[], spread?: number): string;
export function releasesIn(changelog: string): { version: string; codename: string | null; date: string | null }[];
export function usedCodenames(changelog: string): string[];
export function heading(version: string, codename: string, date: string): string;
export function nameRelease(changelog: string, head: string): string;
export function backfill(changelog: string): string;
export function notesFor(changelog: string, version: string): string;
export function title(changelog: string, tag: string): string;
