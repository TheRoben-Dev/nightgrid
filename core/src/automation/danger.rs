//! Spots destructive commands so the UI can ask first (security rule 5:
//! rm, prune, kill, down, reboot, shutdown, upgrade, overwrite).
//!
//! This is a heuristic over shell words, not a parser: it errs towards
//! asking. It looks at every simple command in a line (split on `;`, `|`,
//! `&`, newlines and `$(`/backticks), skips wrappers such as `sudo`, `env`
//! and `nohup`, and matches the command name and its subcommand.

/// Splits into simple commands, each a list of words (quotes removed).
fn commands(line: &str) -> (Vec<Vec<String>>, bool) {
    let mut cmds: Vec<Vec<String>> = vec![Vec::new()];
    let mut word = String::new();
    let mut in_word = false;
    let mut overwrite = false;
    let mut chars = line.chars().peekable();
    let push_word = |cmds: &mut Vec<Vec<String>>, word: &mut String, in_word: &mut bool| {
        if *in_word {
            cmds.last_mut().expect("never empty").push(std::mem::take(word));
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                word.push(n);
                            }
                        }
                        _ => word.push(q),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(n) = chars.next() {
                    word.push(n);
                }
            }
            ';' | '|' | '&' | '\n' | '(' | ')' | '`' | '{' | '}' => {
                push_word(&mut cmds, &mut word, &mut in_word);
                cmds.push(Vec::new());
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                push_word(&mut cmds, &mut word, &mut in_word);
                cmds.push(Vec::new());
            }
            '>' => {
                // A number right before `>` is the fd (`2>`), not an argument.
                if in_word && word.chars().all(|c| c.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                } else {
                    push_word(&mut cmds, &mut word, &mut in_word);
                }
                match chars.peek() {
                    // `>>` appends, `>&2` duplicates a descriptor.
                    Some('>') | Some('&') => {
                        chars.next();
                        let _ = take_target(&mut chars);
                    }
                    _ => {
                        if !take_target(&mut chars).starts_with("/dev/") {
                            overwrite = true;
                        }
                    }
                }
            }
            c if c.is_whitespace() => push_word(&mut cmds, &mut word, &mut in_word),
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    push_word(&mut cmds, &mut word, &mut in_word);
    cmds.retain(|c| !c.is_empty());
    (cmds, overwrite)
}

fn take_target(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    while chars.peek().is_some_and(|c| *c == ' ' || *c == '\t') {
        chars.next();
    }
    let mut t = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || matches!(c, ';' | '|' | '&' | ')') {
            break;
        }
        if c != '\'' && c != '"' {
            t.push(c);
        }
        chars.next();
    }
    t
}

fn base(w: &str) -> &str {
    w.rsplit('/').next().unwrap_or(w)
}

/// Drops wrappers (`sudo -u x`, `env A=1`, `nohup`, `time`, `doas`,
/// `xargs`, `exec`, `timeout 5`) and leading `VAR=value` assignments.
fn strip_wrappers(words: &[String]) -> &[String] {
    let mut i = 0;
    loop {
        let Some(w) = words.get(i) else { return &words[i..] };
        let b = base(w);
        if w.contains('=') && !w.starts_with('-') && i < words.len() - 1 {
            i += 1;
        } else if matches!(
            b,
            "sudo" | "doas" | "env" | "nohup" | "time" | "exec" | "nice" | "ionice" | "xargs" | "command"
        ) {
            i += 1;
            // Their options (and `-u user`).
            while let Some(o) = words.get(i) {
                if o == "-u" || o == "-g" || o == "-n" && b == "nice" {
                    i += 2;
                } else if o.starts_with('-') {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if b == "timeout" {
            i += 1;
            while words.get(i).is_some_and(|o| o.starts_with('-')) {
                i += 1;
            }
            i += 1; // the duration
        } else {
            return &words[i..];
        }
    }
}

fn first_arg(args: &[String]) -> Option<&str> {
    args.iter().map(String::as_str).find(|a| !a.starts_with('-'))
}

fn has(args: &[String], w: &str) -> bool {
    args.iter().any(|a| a == w)
}

/// What a simple command does that needs a confirmation, if anything.
fn classify(words: &[String]) -> Option<String> {
    let words = strip_wrappers(words);
    let (cmd, args) = words.split_first()?;
    let name = base(cmd);
    let sub = first_arg(args).unwrap_or("");
    let label = |s: &str| Some(s.to_owned());
    match name {
        "rm" | "rmdir" | "shred" | "unlink" | "wipefs" | "dd" | "truncate" | "fdisk" | "sfdisk" | "parted"
        | "lvremove" | "vgremove" | "pvremove" => label(name),
        n if n.starts_with("mkfs") => label(n),
        "kill" | "killall" | "pkill" => label(name),
        "reboot" | "shutdown" | "poweroff" | "halt" => label(name),
        "init" | "telinit" if matches!(sub, "0" | "6") => label(&format!("{name} {sub}")),
        "systemctl" => match sub {
            "reboot" | "poweroff" | "halt" | "kexec" | "stop" | "disable" | "mask" | "kill" | "restart" => {
                label(&format!("systemctl {sub}"))
            }
            _ => None,
        },
        "docker" | "podman" | "nerdctl" => {
            if has(args, "prune") {
                return label(&format!("{name} prune"));
            }
            let words: Vec<&str> = args.iter().map(String::as_str).filter(|a| !a.starts_with('-')).collect();
            match words.as_slice() {
                ["compose", rest @ ..] => {
                    let s = rest.iter().find(|w| matches!(**w, "down" | "rm" | "kill" | "stop" | "restart"));
                    s.map(|s| format!("{name} compose {s}"))
                }
                [s, ..] if matches!(*s, "rm" | "rmi" | "kill" | "stop" | "restart") => label(&format!("{name} {s}")),
                ["container" | "image" | "volume" | "network" | "system", s, ..]
                    if matches!(*s, "rm" | "kill" | "stop" | "restart") =>
                {
                    label(&format!("{name} {} {s}", words[0]))
                }
                _ => None,
            }
        }
        "docker-compose" => {
            let s = args.iter().find(|w| matches!(w.as_str(), "down" | "rm" | "kill" | "stop" | "restart"));
            s.map(|s| format!("docker-compose {s}"))
        }
        "apt" | "apt-get" | "dnf" | "yum" | "zypper" | "apk" => match sub {
            "upgrade" | "dist-upgrade" | "full-upgrade" => label(&format!("{name} {sub}")),
            // `apt update` only refreshes lists; on dnf, yum and zypper it upgrades.
            "update" if matches!(name, "dnf" | "yum" | "zypper") => label(&format!("{name} {sub}")),
            "remove" | "purge" | "autoremove" | "erase" | "del" => label(&format!("{name} {sub}")),
            "dup" | "patch" if name == "zypper" => label(&format!("{name} {sub}")),
            _ => None,
        },
        "pacman" | "yay" | "paru" => {
            let flags: Vec<&str> = args.iter().map(String::as_str).filter(|a| a.starts_with('-')).collect();
            if flags.iter().any(|f| f.starts_with("-R")) {
                label(&format!("{name} -R"))
            } else if flags.iter().any(|f| f.starts_with("-S") && f.contains('u')) {
                label(&format!("{name} upgrade"))
            } else {
                None
            }
        }
        "zfs" | "zpool" if matches!(sub, "destroy" | "rollback") => label(&format!("{name} {sub}")),
        "find" if has(args, "-delete") || has(args, "-exec") && has(args, "rm") => label("find -delete"),
        "git" if sub == "clean" || (sub == "reset" && has(args, "--hard")) => label(&format!("git {sub}")),
        _ => None,
    }
}

/// Destructive actions in `script`, deduplicated, in order of appearance.
pub fn destructive(script: &str) -> Vec<String> {
    let (cmds, overwrite) = commands(script);
    let mut out: Vec<String> = Vec::new();
    for c in &cmds {
        if let Some(r) = classify(c) {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    if overwrite {
        out.push("overwrite (>)".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harmless() {
        for s in [
            "uptime",
            "df -h",
            "docker ps -a",
            "systemctl status nginx",
            "echo rm -rf /",
            "grep -r 'kill' /var/log",
            "apt list --upgradable",
            "journalctl -u x > /dev/null 2>&1",
            "ls >> /tmp/log",
            "docker compose ps",
            "cat a | sort | uniq -c",
            "apt update",
        ] {
            assert!(destructive(s).is_empty(), "{s}: {:?}", destructive(s));
        }
    }

    #[test]
    fn destructive_ones() {
        let cases: &[(&str, &[&str])] = &[
            ("rm -rf /tmp/x", &["rm"]),
            ("sudo -u root /bin/rm x", &["rm"]),
            ("cd /opt && docker compose down", &["docker compose down"]),
            ("docker system prune -af", &["docker prune"]),
            ("docker rm -f web", &["docker rm"]),
            ("docker container stop web", &["docker container stop"]),
            ("sudo systemctl restart nginx", &["systemctl restart"]),
            ("sudo reboot", &["reboot"]),
            ("shutdown -h now", &["shutdown"]),
            ("DEBIAN_FRONTEND=noninteractive apt-get -y upgrade", &["apt-get upgrade"]),
            ("dnf update -y", &["dnf update"]),
            ("pacman -Syu --noconfirm", &["pacman upgrade"]),
            ("pkill -f worker; echo done", &["pkill"]),
            ("echo hi > /etc/motd", &["overwrite (>)"]),
            ("x=$(rm a)", &["rm"]),
            ("find /tmp -name '*.log' -delete", &["find -delete"]),
            ("timeout 10 kill 1", &["kill"]),
            ("ls; rm a; rm b", &["rm"]),
        ];
        for (s, want) in cases {
            assert_eq!(destructive(s), want.iter().map(|w| w.to_string()).collect::<Vec<_>>(), "{s}");
        }
    }
}
