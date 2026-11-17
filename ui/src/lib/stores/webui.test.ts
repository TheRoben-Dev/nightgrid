import { describe, expect, it } from "vitest";
import { targetLabel, targetUrl } from "./webui.svelte";

const t = (port: number, scheme = "http", path = "/", bind: string | null = null) => ({ port, scheme, path, bind, labelled: false });

describe("web UI targets", () => {
  it("builds the URL like core's url_for", () => {
    expect(targetUrl(t(8080), "nas.example.com")).toBe("http://nas.example.com:8080/");
    expect(targetUrl(t(443, "https", "/admin"), "nas")).toBe("https://nas/admin");
    expect(targetUrl(t(80), "fd00::5")).toBe("http://[fd00::5]/");
    expect(targetUrl(t(9000, "http", "/", "127.0.0.1"), "nas")).toBe("http://127.0.0.1:9000/");
  });
  it("labels short", () => {
    expect(targetLabel(t(8080))).toBe("8080");
    expect(targetLabel(t(9443, "https", "/ui"))).toBe("https 9443 /ui");
  });
});
