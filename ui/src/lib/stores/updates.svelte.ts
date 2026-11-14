import { api, errorText, type UpdateInfo, type UpdateStatus } from "../api";
import { settings } from "./settings.svelte";

/** Delay before the launch check, so startup isn't competing with it. */
const LAUNCH_DELAY_MS = 8000;

class UpdatesStore {
  status = $state<UpdateStatus | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** Update shown in the prompt (null = closed). */
  prompt = $state<UpdateInfo | null>(null);
  installing = $state(false);
  /** 0–1 while downloading, null when unknown. */
  progress = $state<number | null>(null);
  phase = $state<"idle" | "downloading" | "installing">("idle");

  /** On launch: one quiet check when enabled; prompts unless that version was skipped. */
  startLaunchCheck() {
    setTimeout(() => {
      if (settings.value.updateCheck) void this.check(false);
    }, LAUNCH_DELAY_MS);
  }

  /** `manual` shows errors and prompts even for a skipped version. */
  async check(manual: boolean) {
    if (this.checking) return;
    this.checking = true;
    this.error = null;
    try {
      const s = await api.updateCheck();
      this.status = s;
      if (s.kind === "available" && (manual || settings.value.updateSkipVersion !== s.version)) {
        this.prompt = s;
      }
    } catch (e) {
      if (manual) this.error = errorText(e);
    } finally {
      this.checking = false;
    }
  }

  later() {
    this.prompt = null;
  }

  skip() {
    if (this.prompt) void settings.update({ updateSkipVersion: this.prompt.version });
    this.prompt = null;
  }

  async install() {
    if (this.installing) return;
    this.installing = true;
    this.error = null;
    this.phase = "downloading";
    this.progress = null;
    try {
      await api.updateInstall((p) => {
        if (p.kind === "downloading") {
          this.progress = p.total ? Math.min(1, p.done / p.total) : null;
        } else {
          this.phase = "installing";
          this.progress = 1;
        }
      });
      // The app restarts on success; nothing to do here.
    } catch (e) {
      this.error = errorText(e);
      this.phase = "idle";
    } finally {
      this.installing = false;
    }
  }
}

export const updates = new UpdatesStore();
