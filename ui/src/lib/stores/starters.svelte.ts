/** The starter catalog (D-177), read once from the app and shared by Fleet, Knowledge and alerts. */
import { api, errorText, type StarterCatalog } from "../api";

class StartersStore {
  catalog = $state<StarterCatalog | null>(null);
  error = $state<string | null>(null);
  /** A starter runbook to show when Knowledge → Starters opens. */
  focus = $state<string | null>(null);
  #loading: Promise<void> | null = null;

  load(): Promise<void> {
    this.#loading ??= api
      .startersList()
      .then((c) => {
        this.catalog = c;
        this.error = null;
      })
      .catch((e) => {
        this.error = errorText(e);
        this.#loading = null;
      });
    return this.#loading;
  }
}

export const starters = new StartersStore();
