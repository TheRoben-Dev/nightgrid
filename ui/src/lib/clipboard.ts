import { inTauri } from "./api";

/** Clipboard text via the Tauri plugin (no WebKit paste popup), or the web API. */
export async function readClipboard(): Promise<string> {
  if (inTauri()) {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return (await readText()) ?? "";
  }
  return navigator.clipboard?.readText?.() ?? "";
}

export async function writeClipboard(text: string): Promise<void> {
  if (inTauri()) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
    return;
  }
  await navigator.clipboard?.writeText?.(text);
}

/** Opens an http(s) link in the system browser (never inside the app). */
export async function openExternal(url: string): Promise<void> {
  if (!/^https?:\/\//i.test(url)) return;
  if (inTauri()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
