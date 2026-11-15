/** Config drift state (M10l): System → CONFIG for the selected host. */
import { api, errorText, type DriftReport, type FileDiff, type SideInfo } from "../api";

class ConfdiffStore {
  hostId = $state<number | null>(null);
  /** `null` compares with the saved baseline; a host id compares with that host. */
  other = $state<number | null>(null);
  hideHostSpecific = $state(true);
  baseline = $state<SideInfo | null>(null);
  baselineLoaded = $state(false);
  report = $state<DriftReport | null>(null);
  loading = $state(false);
  saving = $state(false);
  confirmSave = $state(false);
  error = $state<string | null>(null);
  /** The file whose diff is open, and the diff. */
  openPath = $state<string | null>(null);
  diff = $state<FileDiff | null>(null);
  diffLoading = $state(false);
  diffError = $state<string | null>(null);
  private seq = 0;

  /** Switches to a host: forgets the last result and reads its baseline. */
  async select(hostId: number) {
    if (hostId === this.hostId && this.baselineLoaded) return;
    this.seq++;
    this.hostId = hostId;
    if (this.other === hostId) this.other = null;
    this.baseline = null;
    this.baselineLoaded = false;
    this.report = null;
    this.error = null;
    this.closeDiff();
    const seq = this.seq;
    try {
      const b = await api.confdiffBaseline(hostId);
      if (seq !== this.seq) return;
      this.baseline = b;
    } catch (e) {
      if (seq === this.seq) this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.baselineLoaded = true;
    }
  }

  async saveBaseline() {
    const id = this.hostId;
    if (id === null) return;
    this.confirmSave = false;
    this.saving = true;
    try {
      this.baseline = await api.confdiffSaveBaseline(id);
      this.error = null;
      if (this.other === null) this.report = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.saving = false;
    }
  }

  async compare() {
    const id = this.hostId;
    if (id === null) return;
    const seq = ++this.seq;
    this.loading = true;
    this.closeDiff();
    try {
      const r = await api.confdiffCompare(id, this.other, this.hideHostSpecific);
      if (seq !== this.seq) return;
      this.report = r;
      this.error = null;
    } catch (e) {
      if (seq === this.seq) this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  async openFile(path: string) {
    const id = this.hostId;
    if (id === null) return;
    if (this.openPath === path) return this.closeDiff();
    this.openPath = path;
    this.diff = null;
    this.diffError = null;
    this.diffLoading = true;
    try {
      const d = await api.confdiffFile(id, this.other, path);
      if (this.openPath === path) this.diff = d;
    } catch (e) {
      if (this.openPath === path) this.diffError = errorText(e);
    } finally {
      if (this.openPath === path) this.diffLoading = false;
    }
  }

  closeDiff() {
    this.openPath = null;
    this.diff = null;
    this.diffError = null;
    this.diffLoading = false;
  }
}

export const confdiff = new ConfdiffStore();
