import { describe, expect, it } from "vitest";
import type { Alert, Host } from "../api";
import { hostFacts, hostScore, mentionedHosts } from "./hostFacts";
import { contextBlock } from "./prompt";

const host = (id: number, name: string, extra: Partial<Host> = {}): Host => ({
  id,
  name,
  hostname: `${name.toLowerCase().replace(/\s+/g, "-")}.lan`,
  port: 22,
  username: null,
  groupId: null,
  color: null,
  icon: null,
  favorite: false,
  notes: null,
  authMethod: "agent",
  identityFile: null,
  proxyJump: null,
  rememberSecret: false,
  sshAlias: null,
  tailnetAddress: null,
  driftHistory: false,
  tags: [],
  ...extra,
});

describe("which hosts a question names", () => {
  const hs = [host(1, "Garage Pi"), host(2, "lumen"), host(3, "Main Server"), host(4, "nas")];
  it("matches whole names, loosely", () => {
    expect(mentionedHosts(hs, "what happened to Garage Pi?").map((h) => h.id)).toEqual([1]);
    expect(mentionedHosts(hs, "what happened to the garage-pi").map((h) => h.id)).toEqual([1]);
    expect(mentionedHosts(hs, "is Lumen up").map((h) => h.id)).toEqual([2]);
    expect(mentionedHosts(hs, "nas disk full?").map((h) => h.id)).toEqual([4]);
  });
  it("matches a distinctive word and one typo", () => {
    expect(hostScore(hs[0], "why is my garage offline")).toBe(1);
    expect(hostScore(hs[1], "lumem logs")).toBe(1);
  });
  it("ignores common words", () => {
    expect(mentionedHosts(hs, "how do I add a server")).toEqual([]);
    expect(mentionedHosts(hs, "how do I open a terminal")).toEqual([]);
  });
});

describe("host facts", () => {
  const now = 1_800_000_000;
  const alert: Alert = {
    id: 1,
    key: "offline:1",
    kind: "hostOffline",
    severity: "warn",
    title: "Garage Pi is offline",
    detail: "no answer on port 22",
    hostId: 1,
    since: now - 3600,
    resolved: now - 600,
  };
  it("lists status, last seen, alerts and history", () => {
    const text = hostFacts({
      host: host(1, "Garage Pi", { driftHistory: true, tailnetAddress: "100.64.0.9" }),
      ping: { id: 1, state: "offline", latencyMs: null, connected: false },
      alerts: [alert],
      history: [{ command: "sudo systemctl suspend", shell: "bash", when: null }],
      now,
    });
    expect(text).toContain("### Garage Pi");
    expect(text).toContain("tailnet fallback 100.64.0.9");
    expect(text).toContain("Reachability check: offline");
    expect(text).toContain("Garage Pi is offline");
    expect(text).toContain("10 min ago");
    expect(text).toContain("- sudo systemctl suspend");
  });
  it("says when history isn't shared, and the prompt carries the block first", () => {
    const text = hostFacts({ host: host(2, "lumen"), alerts: [], now });
    expect(text).toContain("not shared");
    const block = contextBlock({ view: "overview", hosts: [text] }, []);
    expect(block).toContain("MY HOSTS");
    expect(block.indexOf("MY HOSTS")).toBeLessThan(block.indexOf("lumen") + 1);
  });
});
