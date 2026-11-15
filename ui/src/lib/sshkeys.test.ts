import { describe, expect, it } from "vitest";
import type { AuthorizedKey, Host, SshKey } from "./api";
import {
  accepted,
  freeKeyName,
  homeOf,
  hostsAccepting,
  keyForHost,
  keyLabel,
  rotationSteps,
  shortFp,
  switchedInput,
  unknownKeys,
  validKeyName,
  type HostKeys,
} from "./sshkeys";

const key = (fingerprint: string, file: string | null, comment = ""): SshKey => ({
  fingerprint,
  algorithm: "ssh-ed25519",
  comment,
  public: `ssh-ed25519 AAAA ${comment}`,
  file,
  encrypted: false,
  inAgent: false,
});
const entry = (fingerprint: string, line = 1): AuthorizedKey => ({ line, fingerprint, algorithm: "ssh-ed25519", comment: "", options: null });

describe("ssh keys", () => {
  const keys = [key("SHA256:aaaaaaaaaaaaaaaaaaaaaa", "/home/me/.ssh/id_ed25519"), key("SHA256:bbb", null, "agent@example")];

  it("shortens fingerprints and labels keys", () => {
    expect(shortFp("SHA256:aaaaaaaaaaaaaaaaaaaaaa")).toBe("SHA256:aaaaaaaa…aaaa");
    expect(shortFp("SHA256:short")).toBe("SHA256:short");
    expect(keyLabel(keys[0])).toBe("id_ed25519");
    expect(keyLabel(keys[1])).toBe("agent@example");
    expect(keyLabel(key("SHA256:x", "C:\\Users\\me\\.ssh\\lab"))).toBe("lab");
  });

  it("finds the key a host uses, with ~ expanded", () => {
    const home = homeOf("/home/me/.ssh");
    expect(home).toBe("/home/me");
    expect(keyForHost({ identityFile: "~/.ssh/id_ed25519" }, keys, home)?.fingerprint).toBe(keys[0].fingerprint);
    expect(keyForHost({ identityFile: "/home/me/.ssh//id_ed25519" }, keys, home)).toBeDefined();
    expect(keyForHost({ identityFile: "~/.ssh/other" }, keys, home)).toBeUndefined();
    expect(keyForHost({ identityFile: null }, keys, home)).toBeUndefined();
    expect(homeOf("C:\\Users\\me\\.ssh")).toBe("C:\\Users\\me");
  });

  it("works out which hosts accept which keys", () => {
    const results: HostKeys[] = [
      { hostId: 1, entries: [entry("SHA256:aaaaaaaaaaaaaaaaaaaaaa"), entry("SHA256:zzz", 2), entry("SHA256:zzz", 3)], error: null },
      { hostId: 2, entries: null, error: "offline" },
      { hostId: 3, entries: [entry("SHA256:bbb")], error: null },
    ];
    expect([...accepted(results[0])]).toEqual(["SHA256:aaaaaaaaaaaaaaaaaaaaaa", "SHA256:zzz"]);
    expect(accepted(results[1]).size).toBe(0);
    expect(unknownKeys(results[0], keys).map((e) => e.fingerprint)).toEqual(["SHA256:zzz"]);
    expect(hostsAccepting(results, "SHA256:aaaaaaaaaaaaaaaaaaaaaa")).toEqual([1]);
    expect(hostsAccepting(results, "SHA256:bbb")).toEqual([3]);
  });

  it("switches a host to a key file", () => {
    const h = { id: 4, port: 22, tags: ["lab"], authMethod: "password", identityFile: null } as unknown as Host;
    const i = switchedInput(h, "/home/me/.ssh/new");
    expect(i.port).toBeNull();
    expect(i.identityFile).toBe("/home/me/.ssh/new");
    expect(i.authMethod).toBe("key");
    expect(switchedInput({ ...h, authMethod: "agent" } as Host, "x").authMethod).toBe("agent");
  });

  it("names new keys and plans rotations", () => {
    expect(freeKeyName(keys)).toBe("id_ed25519_nightgrid");
    expect(freeKeyName([key("x", "/h/.ssh/id_ed25519_nightgrid"), key("y", "/h/.ssh/id_ed25519_nightgrid-2")])).toBe("id_ed25519_nightgrid-3");
    expect(validKeyName("id_ed25519_2026")).toBe(true);
    for (const bad of ["", ".x", "a b", "a/b", "x.pub"]) expect(validKeyName(bad)).toBe(false);
    expect(rotationSteps({ switchHosts: true, removeOld: true })).toEqual(["push", "verify", "switch", "remove"]);
    expect(rotationSteps({ switchHosts: false, removeOld: false })).toEqual(["push", "verify"]);
  });
});
