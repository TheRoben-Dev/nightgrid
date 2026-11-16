/**
 * Command check (M10p): flags risky shell patterns before a command runs.
 * Rule based and exact, so DRIFT can explain the flags instead of guessing
 * them, and the UI can show them even when the model is wrong.
 */

export type Level = "danger" | "caution" | "info";

export interface Finding {
  level: Level;
  /** The part of the command the finding is about. */
  what: string;
  why: string;
}

/** Splits a command line into simple commands at `|`, `;`, `&&`, `||` and `&`, keeping quotes. */
export function segments(line: string): { words: string[]; pipedFrom: boolean }[] {
  const out: { words: string[]; pipedFrom: boolean }[] = [];
  let words: string[] = [];
  let cur = "";
  let has = false;
  let quote: string | null = null;
  let piped = false;
  const word = () => {
    if (has) words.push(cur);
    cur = "";
    has = false;
  };
  const end = (nextPiped: boolean) => {
    word();
    if (words.length) out.push({ words, pipedFrom: piped });
    words = [];
    piped = nextPiped;
  };
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) {
      if (c === quote) quote = null;
      else if (c === "\\" && quote === '"' && i + 1 < line.length) cur += line[++i];
      else cur += c;
      continue;
    }
    if (c === "'" || c === '"') {
      quote = c;
      has = true;
    } else if (c === "\\" && i + 1 < line.length) {
      cur += line[++i];
      has = true;
    } else if (c === " " || c === "\t" || c === "\n") {
      word();
    } else if (c === "|" || c === ";" || c === "&") {
      const two = line[i + 1] === c;
      if (c === "|" && line[i + 1] === "&") i++;
      if (two) i++;
      end(c === "|" && !two);
    } else if (c === "#" && !has) {
      break;
    } else {
      cur += c;
      has = true;
    }
  }
  end(false);
  return out;
}

const WRAPPERS = new Set(["sudo", "doas", "nohup", "time", "nice", "ionice", "exec", "command", "builtin", "stdbuf", "env", "xargs", "watch", "timeout"]);

/** Drops `sudo`, `env X=1`, `timeout 5` and the like so the real program comes first. */
function strip(words: string[]): { words: string[]; sudo: boolean } {
  let i = 0;
  let sudo = false;
  while (i < words.length) {
    const w = words[i];
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(w)) {
      i++;
      continue;
    }
    const base = w.split("/").pop() ?? w;
    if (!WRAPPERS.has(base)) break;
    if (base === "sudo" || base === "doas") sudo = true;
    i++;
    // Options of the wrapper (sudo -u root, timeout 5, nice -n 10).
    while (i < words.length && (words[i].startsWith("-") || /^\d+[smhd]?$/.test(words[i]))) {
      const opt = words[i];
      i++;
      if ((base === "sudo" && /^-[ugCDhpRrT]$/.test(opt)) || (base === "nice" && opt === "-n") || (base === "xargs" && /^-[nIPLsd]$/.test(opt))) i++;
    }
  }
  return { words: words.slice(i), sudo };
}

/** Short flags as single letters (`-rf` → r, f) and long flags as written. */
function flags(args: string[]): Set<string> {
  const s = new Set<string>();
  for (const a of args) {
    if (a === "--") break;
    if (a.startsWith("--")) s.add(a.split("=")[0]);
    else if (/^-[A-Za-z]+$/.test(a)) for (const ch of a.slice(1)) s.add(ch);
  }
  return s;
}

const operands = (args: string[]) => args.filter((a) => !a.startsWith("-"));
const SYSTEM_PATH = /^(\/|\/\*|~\/?|\$HOME\/?|\/(bin|boot|dev|etc|home|lib|lib64|opt|root|sbin|srv|usr|var)\/?\*?)$/;
const BLOCK_DEV = /^\/dev\/(sd[a-z]|nvme\d|hd[a-z]|vd[a-z]|xvd[a-z]|mmcblk\d|disk\d|md\d|dm-\d|mapper\/)/;
const SSH_UNITS = /^(ssh|sshd|openssh-server|ssh\.socket|sshd\.service|ssh\.service)$/;

function checkOne(words: string[], pipedFrom: boolean, out: Finding[]) {
  const { words: w, sudo } = strip(words);
  if (!w.length) return;
  const base = (w[0].split("/").pop() ?? w[0]).toLowerCase();
  const prog = base.startsWith("mkfs") ? "mkfs" : base;
  const args = w.slice(1);
  const f = flags(args);
  const ops = operands(args);
  const sub = ops[0]?.toLowerCase() ?? "";
  const add = (level: Level, what: string, why: string) => out.push({ level, what, why });
  const cmd = w.join(" ");

  if (sudo) add("info", "sudo", "Runs as root: mistakes aren't stopped by file permissions.");

  if (pipedFrom && ["sh", "bash", "zsh", "dash", "ksh", "fish", "python", "python3", "perl", "ruby", "node"].includes(prog)) {
    add("danger", `| ${prog}`, "Runs a script straight from the previous command (often a download) without letting you read it first. Download it, read it, then run it.");
  }

  switch (prog) {
    case "rm": {
      const rec = f.has("r") || f.has("R") || f.has("--recursive");
      const force = f.has("f") || f.has("--force");
      if (f.has("--no-preserve-root")) add("danger", "--no-preserve-root", "Allows deleting the whole filesystem starting at /.");
      const sys = ops.filter((o) => SYSTEM_PATH.test(o));
      if (rec && sys.length) add("danger", `rm -r ${sys.join(" ")}`, "Deletes a system or home folder and everything inside it. There is no trash on a server.");
      else if (rec && force) add("danger", "rm -rf", "Deletes folders and everything in them without asking, even read-only files. Double-check the path; there is no undo.");
      else if (rec) add("caution", "rm -r", "Deletes folders and everything in them.");
      else add("caution", "rm", "Deletes files for good (no trash).");
      if (ops.some((o) => /^\$\{?[A-Za-z_]/.test(o) || o.includes("/$"))) add("danger", ops.find((o) => o.includes("$")) ?? "$VAR", "Path built from a variable: if it's empty this can point at a much bigger folder (rm -rf $DIR/ becomes rm -rf /).");
      if (ops.includes("*") || ops.some((o) => o.endsWith("/*"))) add("caution", "*", "Wildcard: matches everything in the folder you're in. Run `ls` with the same pattern first.");
      break;
    }
    case "shred":
    case "wipefs":
    case "blkdiscard":
      add("danger", prog, "Destroys data on the target beyond recovery.");
      break;
    case "dd": {
      const of = args.find((a) => a.startsWith("of="));
      if (of && BLOCK_DEV.test(of.slice(3))) add("danger", of, "Writes over a whole disk or partition. A wrong device name wipes the wrong disk; check with `lsblk` first.");
      else if (of) add("caution", of, "Overwrites the output file.");
      break;
    }
    case "fdisk":
    case "sfdisk":
    case "gdisk":
    case "sgdisk":
    case "parted":
    case "cfdisk":
      add("danger", prog, "Changes the partition table: the wrong disk or a typo can make data unreachable.");
      break;
    case "chmod": {
      const mode = ops[0] ?? "";
      if (/^0?777$/.test(mode) || /^(a|ugo)?\+rwx$/.test(mode) || /o\+w/.test(mode)) add("danger", `chmod ${mode}`, "Lets every user (and every compromised service) change these files. Give access to the right user or group instead.");
      if ((f.has("R") || f.has("--recursive")) && ops.slice(1).some((o) => SYSTEM_PATH.test(o))) add("danger", "chmod -R /…", "Changes permissions across a system folder; this can break sudo, SSH and packages.");
      else if (f.has("R") || f.has("--recursive")) add("caution", "chmod -R", "Changes every file below the folder.");
      break;
    }
    case "chown":
    case "chgrp":
      if ((f.has("R") || f.has("--recursive")) && ops.slice(1).some((o) => SYSTEM_PATH.test(o))) add("danger", `${prog} -R /…`, "Changes owners across a system folder; this can break sudo, SSH and packages.");
      else if (f.has("R") || f.has("--recursive")) add("caution", `${prog} -R`, "Changes every file below the folder.");
      break;
    case "find":
      if (args.includes("-delete")) add("danger", "find -delete", "Deletes everything that matches. Run it once without -delete to see the list.");
      if (args.includes("-exec") && args.some((a) => /^(rm|shred)$/.test(a))) add("danger", "find -exec rm", "Deletes everything that matches. Run it once with -print first.");
      break;
    case "truncate":
      add("caution", "truncate", "Cuts files to the given size; the cut part is gone.");
      break;
    case "mv":
      if (ops.at(-1) === "/dev/null") add("danger", "mv … /dev/null", "Throws the file away (and can replace /dev/null when run as root).");
      else if (!f.has("n") && !f.has("i") && !f.has("--no-clobber")) add("info", "mv", "Replaces a file of the same name at the target without asking (add -i or -n to be asked).");
      break;
    case "crontab":
      if (f.has("r")) add("danger", "crontab -r", "Deletes all of this user's cron jobs at once, without asking. `crontab -l > backup` first.");
      break;
    case "sed":
    case "perl":
      if (f.has("i") || args.some((a) => a.startsWith("-i") || a.startsWith("--in-place"))) add("caution", `${prog} -i`, "Edits the file in place. Keep a copy (sed -i.bak) until you've checked the result.");
      break;
    case "kill":
    case "pkill":
    case "killall": {
      if (f.has("9") || args.includes("-KILL") || args.includes("-SIGKILL") || args.some((a) => /^-s$/.test(a))) add("caution", `${prog} -9`, "Kills without letting the program save or clean up. Try a plain kill (SIGTERM) first.");
      if (prog === "kill" && (ops.includes("1") || args.includes("-1"))) add("danger", "kill 1 / kill -1", "Signals init or every process you own: this can take the whole machine or session down.");
      if (prog !== "kill") add("caution", prog, "Matches processes by name: it can hit more processes than you expect. `pgrep -a` with the same pattern shows which.");
      break;
    }
    case "shutdown":
    case "reboot":
    case "poweroff":
    case "halt":
      add("danger", prog, "Takes the host down: everything on it stops, and over SSH you lose the connection.");
      break;
    case "init":
    case "telinit":
      if (ops[0] === "0" || ops[0] === "6") add("danger", `${prog} ${ops[0]}`, "Shuts down or reboots the host.");
      break;
    case "systemctl": {
      const units = ops.slice(1);
      if (["reboot", "poweroff", "halt", "kexec", "rescue", "emergency"].includes(sub)) add("danger", `systemctl ${sub}`, "Takes the host down or out of normal operation; SSH goes with it.");
      if (["stop", "disable", "mask", "kill"].includes(sub) && units.some((u) => SSH_UNITS.test(u))) add("danger", `systemctl ${sub} ${units.find((u) => SSH_UNITS.test(u))}`, "Stops SSH: you can lock yourself out of this host.");
      else if (["stop", "restart", "disable", "mask", "kill"].includes(sub)) add("caution", `systemctl ${sub}`, "Stops or restarts a service; whatever depends on it goes down for a moment.");
      if (["daemon-reexec", "isolate"].includes(sub)) add("caution", `systemctl ${sub}`, "Changes how systemd itself runs.");
      break;
    }
    case "service":
      if (ops.length >= 2 && SSH_UNITS.test(ops[0]) && /stop|restart/.test(ops[1])) add("danger", `service ${ops[0]} ${ops[1]}`, "Stops SSH: you can lock yourself out.");
      break;
    case "iptables":
    case "ip6tables":
      if (f.has("F") || f.has("--flush")) add("danger", `${prog} -F`, "Removes every firewall rule; with a DROP default policy this cuts you off, otherwise it opens everything.");
      if (args.includes("-P") && args.some((a) => a === "DROP" || a === "REJECT")) add("danger", `${prog} -P … DROP`, "A DROP default policy blocks your own SSH unless an ACCEPT rule comes first.");
      break;
    case "nft":
      if (/flush\s+ruleset/.test(cmd)) add("danger", "nft flush ruleset", "Removes every firewall rule at once.");
      break;
    case "ufw":
      if (sub === "reset") add("danger", "ufw reset", "Deletes all ufw rules and turns the firewall off.");
      if (sub === "enable") add("caution", "ufw enable", "Turns the firewall on: make sure SSH is allowed (`ufw allow OpenSSH`) or you'll be locked out.");
      if (sub === "deny" && /\b22\b|ssh/i.test(cmd)) add("danger", "ufw deny ssh", "Blocks SSH: you can lock yourself out.");
      break;
    case "mkfs":
      add("danger", base, "Formats the target: everything on it is erased.");
      break;
    case "zfs":
    case "zpool":
      if (sub === "destroy") add("danger", `${prog} destroy`, "Destroys the dataset or pool and its data, and with -r every snapshot too.");
      if (prog === "zfs" && sub === "rollback") add("danger", "zfs rollback", "Throws away everything written since the snapshot (with -r, also newer snapshots).");
      break;
    case "lvremove":
    case "vgremove":
    case "pvremove":
      add("danger", prog, "Removes LVM volumes; the data on them is gone.");
      break;
    case "mdadm":
      if (f.has("--zero-superblock") || f.has("--stop") || f.has("--fail") || f.has("--remove")) add("danger", "mdadm", "Changes a RAID array; on the wrong device it degrades or destroys it.");
      break;
    case "docker":
    case "podman": {
      const rest = ops.slice(1).map((o) => o.toLowerCase());
      if ((sub === "system" || sub === "image" || sub === "container" || sub === "network" || sub === "builder") && rest[0] === "prune") {
        if (f.has("a") || f.has("--all")) add("danger", `${prog} ${sub} prune -a`, "Removes every image not used by a running container; anything stopped has to be pulled again.");
        if (f.has("--volumes")) add("danger", "--volumes", "Also deletes unused volumes, which is where container data lives.");
        if (!f.has("a") && !f.has("--all") && !f.has("--volumes")) add("caution", `${prog} ${sub} prune`, "Removes stopped containers, unused networks and dangling images.");
      }
      if (sub === "volume" && (rest[0] === "rm" || rest[0] === "prune")) add("danger", `${prog} volume ${rest[0]}`, "Deletes volumes and the data in them (databases, uploads).");
      if (sub === "compose" || sub === "stack") {
        if (rest[0] === "down" && (f.has("v") || f.has("--volumes"))) add("danger", "compose down -v", "Also deletes the stack's named volumes: the data is gone.");
        else if (rest[0] === "down") add("caution", "compose down", "Stops and removes the stack's containers (volumes stay).");
      }
      if ((sub === "rm" || (sub === "container" && rest[0] === "rm")) && (f.has("f") || f.has("--force"))) add("caution", `${prog} rm -f`, "Removes running containers without stopping them cleanly.");
      if (sub === "rm" && f.has("v")) add("danger", `${prog} rm -v`, "Also deletes the container's anonymous volumes.");
      if (sub === "rmi") add("caution", `${prog} rmi`, "Removes images; they have to be pulled again.");
      if (sub === "run" && f.has("--privileged")) add("danger", "--privileged", "Gives the container full access to the host's devices and kernel.");
      if (sub === "run" && args.some((a) => a.includes("/var/run/docker.sock") || a.includes("/run/docker.sock"))) add("caution", "docker.sock", "The container can control Docker, which is the same as root on the host.");
      if (sub === "run" && args.some((a) => /^\/:/.test(a))) add("danger", "-v /:…", "Mounts the host's whole filesystem into the container.");
      break;
    }
    case "git":
      if (sub === "push" && (f.has("f") || f.has("--force"))) add("danger", "git push --force", "Replaces the remote branch and can erase other people's commits. Use --force-with-lease.");
      if (sub === "reset" && f.has("--hard")) add("danger", "git reset --hard", "Throws away uncommitted changes in tracked files.");
      if (sub === "clean" && (f.has("f") || f.has("--force"))) add("danger", "git clean -f", "Deletes untracked files (with -x also ignored ones like .env). Try -n first.");
      if (sub === "checkout" && ops.includes(".")) add("caution", "git checkout .", "Discards uncommitted changes.");
      break;
    case "apt":
    case "apt-get":
    case "dnf":
    case "yum":
    case "zypper":
    case "pacman":
      if (["remove", "purge", "autoremove", "erase"].includes(sub) || f.has("R")) add("caution", `${prog} ${sub || "-R"}`, "Removes packages; check the list it prints, it can take dependencies along (removing the wrong one can remove SSH or the kernel).");
      if (["dist-upgrade", "full-upgrade", "upgrade", "distro-sync"].includes(sub) || (prog === "pacman" && f.has("u"))) add("caution", `${prog} ${sub}`, "Upgrades packages; services restart and a reboot may be needed.");
      if (f.has("y") || f.has("--yes") || f.has("--assume-yes") || f.has("--noconfirm")) add("caution", "-y", "Answers yes for you, so you won't see the list of changes before they happen.");
      break;
    case "rpm-ostree":
      if (["rebase", "reset", "uninstall"].includes(sub)) add("caution", `rpm-ostree ${sub}`, "Changes the deployed system image; it applies on the next boot (rollback with `rpm-ostree rollback`).");
      break;
    case "qm":
    case "pct":
      if (sub === "destroy") add("danger", `${prog} destroy`, "Deletes the VM or container and its disks.");
      if (sub === "stop") add("caution", `${prog} stop`, "Pulls the plug on the guest (no clean shutdown); use shutdown instead.");
      break;
    case "virsh":
      if (sub === "undefine" && args.some((a) => a.startsWith("--remove-all-storage") || a === "--storage")) add("danger", "virsh undefine --remove-all-storage", "Deletes the VM and its disk images.");
      else if (sub === "destroy") add("caution", "virsh destroy", "Pulls the plug on the VM (it isn't deleted); use shutdown for a clean stop.");
      break;
    case "passwd":
      if (f.has("d")) add("danger", "passwd -d", "Removes the password: the account may log in without one.");
      break;
    case "userdel":
      add("caution", "userdel", f.has("r") ? "Deletes the user and their home folder." : "Deletes the user account.");
      break;
    case "history":
      if (f.has("c")) add("info", "history -c", "Clears this shell's history.");
      break;
  }

  // Generic flags not covered above.
  if ((f.has("--force") || (f.has("f") && ["cp", "ln", "umount", "mount", "zpool", "docker"].includes(prog))) && !out.some((o) => o.what.includes("-f") || o.what.includes("force"))) {
    add("caution", "--force", "Skips the safety checks the command would normally do.");
  }
}

/** Redirections that overwrite (`> file`, but not `>>`, `2>&1` or `/dev/null`). */
function redirects(line: string, out: Finding[]) {
  const re = /(?:^|[^>&0-9])(\d?)>(?!>|&)\s*([^\s;|&]+)/g;
  for (const m of line.matchAll(re)) {
    const target = m[2];
    if (target === "/dev/null") continue;
    if (BLOCK_DEV.test(target)) out.push({ level: "danger", what: `> ${target}`, why: "Writes straight over a disk device: the filesystem on it is destroyed." });
    else if (/^\/etc\/|^\/boot\//.test(target)) out.push({ level: "caution", what: `> ${target}`, why: "Replaces a system file's contents; `>>` appends instead. Keep a copy first." });
    else out.push({ level: "info", what: `> ${target}`, why: "Replaces the file's contents (use >> to append)." });
  }
}

const FORK_BOMB = /:\s*\(\s*\)\s*\{\s*:\s*\|\s*:\s*&\s*\}\s*;\s*:/;

/** Every finding for a command line, most serious first, without duplicates. */
export function checkCommand(line: string): Finding[] {
  const text = line.trim().replace(/^[$#]\s+/, "");
  const out: Finding[] = [];
  if (FORK_BOMB.test(text)) out.push({ level: "danger", what: ":(){ :|:& };:", why: "A fork bomb: starts processes until the machine stops responding." });
  for (const s of segments(text)) checkOne(s.words, s.pipedFrom, out);
  redirects(text, out);
  const rank = { danger: 0, caution: 1, info: 2 };
  const seen = new Set<string>();
  return out.filter((f) => !seen.has(f.level + f.what) && seen.add(f.level + f.what)).sort((a, b) => rank[a.level] - rank[b.level]);
}

/** The highest level among findings, or null for none. */
export const worst = (fs: Finding[]): Level | null => (fs.some((f) => f.level === "danger") ? "danger" : fs.some((f) => f.level === "caution") ? "caution" : fs.length ? "info" : null);

const KNOWN = /^(sudo|doas|rm|cp|mv|ls|cat|chmod|chown|chgrp|find|dd|mkfs(\.\w+)?|kill|pkill|killall|systemctl|service|journalctl|docker|podman|git|apt|apt-get|dnf|yum|zypper|pacman|rpm-ostree|curl|wget|tar|zfs|zpool|iptables|nft|ufw|crontab|sed|awk|grep|ssh|scp|rsync|qm|pct|virsh|shutdown|reboot|poweroff|truncate|shred|ln|mount|umount|echo|printf|bash|sh|python3?|npm|pnpm|pip3?|make|kubectl|helm|lsblk|df|du|ps|top|htop|tail|head|less|vim?|nano|passwd|useradd|userdel|usermod|ip|ss|ping|nc|openssl|wipefs|parted|fdisk|lvremove|vgremove|mdadm|find|xargs|env|nohup|watch|timeout|tee|history)$/;

/** Whether a line looks like a shell command (for picking one out of a question or a selection). */
export function looksLikeCommand(line: string): boolean {
  const t = line.trim().replace(/^[$#]\s+/, "");
  if (!t || t.length > 400 || t.includes("\n")) return false;
  const first = t.split(/\s+/)[0].split("/").pop() ?? "";
  return KNOWN.test(first.replace(/^mkfs\..*/, "mkfs"));
}
