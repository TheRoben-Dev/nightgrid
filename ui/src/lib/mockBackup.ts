/**
 * Browser-only stand-in for encrypted backups: no files are touched; export
 * and open pretend a file was picked, so the Settings panel can be exercised
 * in `pnpm dev`, tests and screenshots.
 */
import type { BackupFolder_Serialize as BackupFolder, BackupImported, BackupPreview, BackupWritten, ImportMode } from "./bindings";

const MIN = 8;
let folder: BackupFolder | null = null;
let picked = false;

const summary = {
  appVersion: "0.9.0",
  createdAt: 1_763_200_000,
  hosts: 4,
  groups: 2,
  tags: 3,
  snippets: 4,
  playbooks: 1,
  services: 6,
  tunnels: 2,
  integrations: 1,
  bookmarks: 3,
  settings: true,
  secrets: 0,
};

export const mockBackup = {
  async export(passphrase: string, includeSecrets: boolean, toFolder: boolean): Promise<BackupWritten | null> {
    if (passphrase.length < MIN) throw new Error(`Use a passphrase of at least ${MIN} characters.`);
    if (toFolder && !folder) throw new Error("Pick a sync folder first.");
    await new Promise((r) => setTimeout(r, 300));
    const path = toFolder && folder ? `${folder.path}/nightgrid-backup.age` : "~/Documents/nightgrid-backup.age";
    if (toFolder && folder) folder = { ...folder, modified: Date.now() / 1000 };
    return { path, secrets: includeSecrets ? 2 : 0, bytes: 5120 };
  },
  async open(passphrase: string, fromFolder: boolean): Promise<BackupPreview | null> {
    if (fromFolder && !folder) throw new Error("Pick a sync folder first.");
    await new Promise((r) => setTimeout(r, 300));
    if (passphrase !== "correct horse") throw new Error("invalid value for backup: wrong passphrase, or the file isn't a passphrase-encrypted backup");
    picked = true;
    return { path: fromFolder && folder ? `${folder.path}/nightgrid-backup.age` : "~/Documents/nightgrid-backup.age", summary };
  },
  async import(_passphrase: string, mode: ImportMode): Promise<BackupImported> {
    if (!picked) throw new Error("Open a backup first.");
    picked = false;
    return mode === "replace"
      ? { added: 26, skipped: 0, secrets: 0, settings: true, secretsError: null }
      : { added: 9, skipped: 17, secrets: 0, settings: false, secretsError: null };
  },
  async pickFolder(): Promise<BackupFolder | null> {
    folder = { path: "~/Sync/nightgrid", modified: null };
    return folder;
  },
  async folder(): Promise<BackupFolder | null> {
    return folder;
  },
  async clearFolder(): Promise<null> {
    folder = null;
    return null;
  },
};
