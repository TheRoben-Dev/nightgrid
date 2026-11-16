/**
 * Browser-only stand-in for the login & access audit (M10u): a full report
 * read as root for host 1, a limited one elsewhere unless "read as root".
 */
import type { AccessReport_Serialize as AccessReport } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
const FP = "SHA256:mockLaptopKeyFingerprint0000000000000000000";

export const mockAccess = {
  async audit(hostId: number, elevate: boolean): Promise<AccessReport> {
    await new Promise((r) => setTimeout(r, 300));
    const t = now();
    const root = hostId === 1 || elevate;
    return {
      hostId,
      at: t,
      asRoot: root,
      loginUser: "admin",
      days: 7,
      logSource: root ? "journal" : "none",
      logins: root
        ? [
            { ts: t - 600, user: "admin", from: "192.168.1.20", method: "publickey", key: FP },
            { ts: t - 86_400, user: "admin", from: "100.64.0.2", method: "publickey", key: FP },
            ...(hostId === 2 ? [{ ts: t - 3 * 86_400, user: "alice", from: "192.168.1.31", method: "password", key: null }] : []),
          ]
        : [],
      failures: root
        ? [
            { from: "203.0.113.7", count: 412, users: ["root", "admin", "ubuntu", "test", "oracle"], last: t - 40 },
            { from: "198.51.100.23", count: 37, users: ["root"], last: t - 7200 },
            { from: "192.168.1.31", count: 2, users: ["alice"], last: t - 3 * 86_400 - 60 },
          ]
        : [],
      failedTotal: root ? 451 : 0,
      sessions: [
        "admin    pts/0        192.168.1.20     Mon Nov 16 05:20   still logged in",
        "admin    pts/1        100.64.0.2       Sun Nov 15 21:02 - 22:40  (01:38)",
        "reboot   system boot  6.8.0-49-generic Sun Nov 15 03:00   still running",
      ],
      fail2ban: hostId === 1 ? "read" : root ? "missing" : "denied",
      jails:
        hostId === 1
          ? [{ name: "sshd", currentlyFailed: 3, totalFailed: 451, currentlyBanned: 1, totalBanned: 9, banned: ["203.0.113.7"] }]
          : [],
      crowdsec: "missing",
      decisions: [],
      users: [
        { name: "root", uid: 0, shell: "/bin/bash", home: "/root", sudo: true, system: false, keys: root ? 0 : 0 },
        { name: "admin", uid: 1000, shell: "/bin/bash", home: "/home/admin", sudo: true, system: false, keys: 2 },
        { name: "alice", uid: 1001, shell: "/bin/zsh", home: "/home/alice", sudo: false, system: false, keys: root ? 1 : 0 },
        { name: "postgres", uid: 112, shell: "/bin/bash", home: "/var/lib/postgresql", sudo: false, system: true, keys: 0 },
      ],
      sudoersReadable: root,
      sudoRules: root
        ? [
            { file: "/etc/sudoers", rule: "root ALL=(ALL:ALL) ALL" },
            { file: "/etc/sudoers", rule: "%sudo ALL=(ALL:ALL) ALL" },
            { file: "/etc/sudoers.d/90-admin", rule: "admin ALL=(ALL) NOPASSWD: ALL" },
          ]
        : [],
      keys: [
        { user: "admin", key: { line: 1, fingerprint: FP, algorithm: "ssh-ed25519", comment: "laptop", options: null } },
        { user: "admin", key: { line: 2, fingerprint: "SHA256:oldDesktopKey00000000000000000000000000000", algorithm: "ssh-rsa", comment: "old-desktop", options: null } },
        ...(root ? [{ user: "alice", key: { line: 1, fingerprint: "SHA256:aliceKey000000000000000000000000000000000", algorithm: "ssh-ed25519", comment: "alice@phone", options: 'from="192.168.1.0/24"' } }] : []),
      ],
      sshdEffective: root,
      sshd: [
        { key: "port", value: "22" },
        { key: "permitrootlogin", value: "prohibit-password" },
        { key: "passwordauthentication", value: hostId === 1 ? "no" : "yes" },
      ],
      findings:
        hostId === 1
          ? [
              { level: "info", text: "1 address banned right now" },
              { level: "info", text: "admin can run anything with sudo without a password" },
            ]
          : root
            ? [
                { level: "bad", text: "Password logins are on, 451 failed attempts in 7 days, and nothing bans them" },
                { level: "warn", text: "1 password login (latest: alice from 192.168.1.31)" },
                { level: "info", text: "admin can run anything with sudo without a password" },
              ]
            : [{ level: "warn", text: "Password logins are on (PasswordAuthentication)" }],
    };
  },
};
