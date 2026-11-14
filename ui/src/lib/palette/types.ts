import type { ActionId } from "../keys";

export interface PaletteCommand {
  id: string;
  title: string;
  /** Group label, e.g. "NAVIGATE", "THEME", "VIEW". */
  group: string;
  keywords?: string[];
  /** Keymap action whose shortcut is shown next to the command. */
  action?: ActionId;
  run: () => void | Promise<void>;
}

export const commandText = (c: PaletteCommand) => [c.group, c.title, ...(c.keywords ?? [])].join(" ");
