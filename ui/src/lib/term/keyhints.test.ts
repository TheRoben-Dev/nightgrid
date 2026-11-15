import { describe, expect, it } from "vitest";
import { KEYMAPS, keymap, keymapDocs, parseHint, programFromCommand } from "./keyhints";

describe("key hints", () => {
  it.each([
    ["htop", "htop"],
    ["sudo htop", "htop"],
    ["sudo -u www-data -n vim /etc/hosts", "vim"],
    ["env TERM=xterm nano notes.txt", "nano"],
    ["EDITOR=vi git commit", null],
    ["nice -n 10 top", "top"],
    ["/usr/bin/less /var/log/syslog", "less"],
    ["man tar", "less"],
    ["journalctl -u nginx", "less"],
    ["journalctl -fu nginx", "follow"],
    ["sudo journalctl --follow", "follow"],
    ["tail -n 50 -f app.log", "follow"],
    ["tail app.log", null],
    ["docker logs -f web", "follow"],
    ["docker compose logs -f", "follow"],
    ["docker ps", null],
    ["git log --oneline", "less"],
    ["systemctl status sshd", "less"],
    ["dmesg | less", "less"],
    ["cat big.log | grep err | less -R", "less"],
    ["htop | tee x", "htop"],
    ["tmux attach", "tmux"],
    ["ls -la", null],
    ["", null],
  ])("%s → %s", (line, want) => {
    expect(programFromCommand(line)).toBe(want);
  });

  it("process names from local terminals map too", () => {
    expect(programFromCommand("nvim")).toBe("vim");
    expect(programFromCommand("zsh")).toBeNull();
  });

  it("only accepts our token and strips anything unsafe", () => {
    const t = "0123456789abcdef";
    expect(parseHint(`${t};sudo htop`, t)).toBe("sudo htop");
    expect(parseHint(`${t};`, t)).toBeNull();
    expect(parseHint(`ffff;htop`, t)).toBeUndefined();
    expect(parseHint(`${t};vim \u001b]52;evil`, t)).toBe("vim 52evil");
  });

  it("every map has hints and DRIFT gets one doc per map", () => {
    for (const m of KEYMAPS) expect(m.hints.length).toBeGreaterThan(0);
    expect(keymap("htop")?.name).toBe("htop");
    expect(keymapDocs()).toHaveLength(KEYMAPS.length);
    expect(keymapDocs()[0].text).toContain("Ctrl+X: exit");
  });
});
