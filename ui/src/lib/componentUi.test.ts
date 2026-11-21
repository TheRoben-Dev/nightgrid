import { describe, expect, it } from "vitest";
import { packProblem, type HostManifest } from "./componentUi";

const host: HostManifest = {
  format: 1,
  built: 100,
  svelte: "5.57.3",
  modules: { "host/lib/api.js": ["api", "errorText"], "host/npm/svelte/src/internal/client/index.js": ["a", "b"] },
  commands: { pveSnapshots: "h1" },
};
const pack = (requires: object, built = 200) => ({ built, requires: JSON.stringify(requires) });
const ok = { svelte: "5.57.3", modules: { "host/lib/api.js": ["api"], "host/npm/svelte/src/internal/client/index.js": ["*"] }, commands: { pveSnapshots: "h1" } };

describe("component pack fit", () => {
  it("accepts a newer pack whose needs are all there", () => {
    expect(packProblem(pack(ok), host)).toBeNull();
  });
  it("refuses everything else", () => {
    expect(packProblem(pack(ok), null)).toMatch(/no host modules/);
    expect(packProblem(pack(ok, 100), host)).toMatch(/as new/);
    expect(packProblem(pack({ ...ok, svelte: "5.58.0" }), host)).toMatch(/Svelte/);
    expect(packProblem(pack({ ...ok, modules: { "host/lib/gone.js": ["x"] } }), host)).toMatch(/doesn't share/);
    expect(packProblem(pack({ ...ok, modules: { "host/lib/api.js": ["api", "renamed"] } }), host)).toMatch(/renamed/);
    expect(packProblem(pack({ ...ok, commands: { pveSnapshots: "h2" } }), host)).toMatch(/pveSnapshots/);
    expect(packProblem(pack({ ...ok, commands: { gone: "h" } }), host)).toMatch(/gone/);
    expect(packProblem({ built: 200, requires: "{" }, host)).toMatch(/unreadable/);
    expect(packProblem(pack(ok), host, ["proxmox@200"], "proxmox")).toMatch(/failed before/);
  });
});
