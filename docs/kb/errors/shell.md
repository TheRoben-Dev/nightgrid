# Errors: shell, bash and scripts

Messages from bash, sh and the core command-line tools: what each one means and what to try first. Paste or attach the message to DRIFT and it matches it against these entries. For writing scripts that fail cleanly, see [Bash scripts that don't bite](kb:linux/bash-scripts).

### command not found
Means: The shell searched every directory in PATH and found no program with that name. Either it isn't installed, it's spelled wrong, or it lives somewhere outside PATH.
Fix: `type <name>` or `command -v <name>` to check. Install the package that provides it (`apt-file search bin/<name>` or `dnf provides '*/<name>'`), or call it by full path. Cron and systemd use a much shorter PATH than your login shell.

### bash: <name>: No such file or directory
Means: You ran a path (with a slash) that doesn't exist, or a script whose #! interpreter doesn't exist on this system.
Fix: `ls -l <path>`. If the file is there, check its first line (`head -1 <file>`): the interpreter path must exist. Windows line endings make `#!/bin/bash\r` fail the same way: `sed -i 's/\r$//' <file>`.

### /bin/bash^M: bad interpreter: No such file or directory
Means: The script has Windows (CRLF) line endings, so the kernel looks for an interpreter literally named `bash\r`.
Fix: `sed -i 's/\r$//' <file>` or `dos2unix <file>`. Set your editor or git (`core.autocrlf=input`) to save LF.

### bad interpreter: Permission denied
Means: The interpreter named on the #! line exists but can't be executed, often because the script sits on a filesystem mounted with `noexec`.
Fix: Run it as `bash <file>`, or move it off the noexec mount (`findmnt -T <file>` shows the options; /tmp is often noexec).

### Permission denied
Means: The current user lacks the rights for that file or action: no execute bit, no read or write permission on the file, or no search (x) permission on a parent directory. SELinux or AppArmor can also deny it.
Fix: `ls -l <file>` and `namei -l <path>` to see every directory on the way. `chmod +x` for scripts. On Fedora/RHEL: `sudo ausearch -m avc -ts recent`. See [Permissions](kb:linux/permissions).

### Operation not permitted
Means: EPERM: the action needs privileges the process doesn't have (it isn't about file mode bits). Typical: changing ownership as non-root, an immutable file (`chattr +i`), a capability dropped in a container, or macOS/SELinux policy.
Fix: Try with sudo; check `lsattr <file>` for the `i` flag; in a container, check its `cap_add`/`security_opt`.

### syntax error near unexpected token `<token>'
Means: bash hit a character it didn't expect at that point: a missing `then`/`do`/`fi`/`done`, an unclosed quote above it, or a script with CRLF line endings.
Fix: Look a few lines above the reported line for the unclosed construct. `bash -n <file>` checks syntax without running it; `shellcheck <file>` points at the exact cause.

### syntax error: unexpected end of file
Means: bash reached the end of the script while a block, quote or here-document was still open.
Fix: Look for a missing `fi`, `done`, `esac`, `}` or a closing quote. A here-document terminator must be alone on its line with no trailing spaces (unless you use `<<-` with tabs).

### unexpected EOF while looking for matching `"'
Means: A double quote was opened and never closed.
Fix: Find the odd quote, usually in the line number reported or above it. Editors with syntax highlighting show the run-on string.

### unary operator expected
Means: A `[ ]` test got an empty value where it needed a word, so `[ $x = foo ]` became `[ = foo ]`.
Fix: Quote variables in tests: `[ "$x" = foo ]`, or use `[[ $x == foo ]]` in bash.

### integer expression expected
Means: `-eq`, `-lt`, `-gt` and friends compare integers, and one side wasn't one (empty, has spaces, a decimal or a newline).
Fix: Print the value (`declare -p x`). Strip whitespace, use `[[ $a == "$b" ]]` for strings, or `bc`/`awk` for decimals.

### too many arguments
Means: A `[ ]` test got more words than it expects, usually because an unquoted variable contained spaces or a glob.
Fix: Quote the variable: `[ -f "$file" ]`.

### unbound variable
Means: The script runs with `set -u` and used a variable that was never set.
Fix: Set it, or give a default with `${VAR:-default}`. For optional arguments use `${1:-}`.

### bad substitution
Means: The shell doesn't understand that `${...}` form. Common when a bash script runs under sh/dash (Debian's /bin/sh), which lacks `${var//a/b}`, `${var:0:3}` and arrays.
Fix: Run it with bash (`#!/usr/bin/env bash` as the first line, and `bash script.sh`, not `sh script.sh`).

### [[: not found
Means: A script using bash's `[[ ]]` was run by sh (dash), which doesn't have it.
Fix: Start the script with `#!/usr/bin/env bash` and run it as `bash <file>` or `./<file>`.

### Argument list too long
Means: E2BIG: a glob expanded to more filenames than the kernel allows on one command line.
Fix: Use `find <dir> -name '*.log' -exec rm {} +` or `find ... -print0 | xargs -0 <cmd>` instead of `cmd *`.

### ambiguous redirect
Means: The target of a `>` or `<` expanded to nothing or to several words (an empty or unquoted variable with spaces).
Fix: Quote it: `> "$logfile"`, and check that the variable is set.

### cannot overwrite existing file
Means: The shell option `noclobber` is on, so `>` refuses to replace an existing file.
Fix: Use `>|` to overwrite deliberately, or `set +o noclobber`.

### Text file busy
Means: ETXTBSY: you tried to write to (or replace in place) a program that is currently running.
Fix: Stop the program first, or install the new binary under a temp name and `mv` it over the old one (rename is allowed while it runs).

### Exec format error
Means: The kernel can't run the file: it was built for another CPU architecture (arm64 vs x86_64), or it's a script with no #! line being executed directly.
Fix: `file <binary>` and `uname -m` to compare. Get the right build, or add a #! line to the script.

### cannot execute binary file: Exec format error
Means: Same as Exec format error: the binary is for another CPU architecture or isn't a Linux executable at all.
Fix: `file <binary>` shows what it is. Download the build for `uname -m` (x86_64/amd64 or aarch64/arm64).

### cannot execute: required file not found
Means: The program exists but its ELF interpreter (dynamic loader) doesn't, typically a glibc binary on a musl system (Alpine) or a 32-bit binary without 32-bit libs.
Fix: `file <binary>` shows the interpreter it wants. Use a matching build (static or musl), or install the compatibility libraries.

### error while loading shared libraries: <lib>: cannot open shared object file: No such file or directory
Means: The program links against a library that isn't installed or isn't on the loader's search path.
Fix: `ldd <binary> | grep 'not found'` lists the missing ones. Install the package that provides it, or for a library in a custom path add it to /etc/ld.so.conf.d/ and run `sudo ldconfig`.

### version `GLIBC_<ver>' not found
Means: The binary was built against a newer glibc than this system has.
Fix: Use a build for your distro release (or a static/musl build), run it in a container, or upgrade the OS. Don't try to replace the system glibc.

### Killed
Means: The process received SIGKILL, most often from the kernel's out-of-memory killer, a cgroup memory limit, or someone running `kill -9`.
Fix: `dmesg -T | grep -i -B2 -A8 oom` shows memory kills. Add RAM or swap, raise the limit, or reduce what the process loads at once.

### Segmentation fault (core dumped)
Means: The program accessed memory it shouldn't and crashed. Usually a bug or mismatched library versions; crashes across many programs point at bad RAM.
Fix: `coredumpctl list` and `coredumpctl info <pid>` for details. Try the previous version; run memtest86+ if unrelated programs segfault too.

### Bus error (core dumped)
Means: SIGBUS: an invalid memory access, typically a memory-mapped file that shrank or sits on a failing or full disk, or an unaligned access on some CPUs.
Fix: Check the disk (`dmesg -T`, `df -h`) and whether another process truncated the file. Repeated bus errors with healthy disks suggest RAM issues.

### Illegal instruction (core dumped)
Means: The CPU met an instruction it doesn't support: a binary built for newer CPU features (AVX2, AVX-512, x86-64-v3) running on an older CPU or in a VM with a limited CPU model.
Fix: Use a generic build, or in Proxmox/libvirt set the VM CPU type to `host`.

### Floating point exception (core dumped)
Means: SIGFPE: usually an integer division by zero in the program.
Fix: It's a bug or bad input (a zero in config where a divisor is expected). Check the input values; report it upstream if the input is valid.

### Aborted (core dumped)
Means: The program called abort(), usually after an internal assertion failed or the C library detected heap corruption.
Fix: Read the line printed just before it (often "double free" or "assertion failed"). `coredumpctl info` gives the backtrace.

### free(): invalid pointer
Means: glibc detected heap corruption in the program and aborted it on purpose.
Fix: A bug in the program or a mismatched library. Try another version; report it upstream with the backtrace from `coredumpctl info`.

### stack smashing detected
Means: The stack protector caught a buffer overflow in the program and killed it.
Fix: A bug (or an exploit attempt) in that program. Update it and report it; don't run it on untrusted input meanwhile.

### Broken pipe
Means: SIGPIPE/EPIPE: the program wrote into a pipe or socket whose reader had already gone away (for example `cmd | head` closing early, or a client that disconnected).
Fix: Often harmless with `head`. For network services it means the client left mid-response; check timeouts on both sides.

### write error: No space left on device
Means: The filesystem the command writes to is full, or out of inodes.
Fix: `df -h <path>` and `df -i <path>`. See [When the disk is full](kb:linux/disk-full).

### No such file or directory
Means: ENOENT: a path in the command doesn't exist. In scripts, often a relative path run from another working directory, or a variable that was empty.
Fix: `ls -l` the exact path; print it first (`echo "$path"`). In cron and systemd, use absolute paths.

### Is a directory
Means: EISDIR: a command expected a file but got a directory (for example `cat dir` or redirecting to a directory).
Fix: Name a file inside it, or use the recursive form of the command (`cp -r`, `rm -r`).

### Not a directory
Means: ENOTDIR: a path component you treated as a folder is a file (`/etc/hosts/foo`).
Fix: Check each part of the path with `namei <path>`.

### Directory not empty
Means: ENOTEMPTY: `rmdir` (or a rename over a directory) needs the directory to be empty.
Fix: Look inside first (`ls -A`), then remove contents or use `rm -r` deliberately.

### File exists
Means: EEXIST: you tried to create something (mkdir, ln, mknod) where a file of that name already exists.
Fix: Use `mkdir -p` for directories that may exist, `ln -sf` to replace a link, or remove the old one first.

### File name too long
Means: ENAMETOOLONG: one path component exceeds 255 bytes (or the full path exceeds 4096). Encrypted home directories (eCryptfs) allow only about 143.
Fix: Shorten the name. Watch out for multi-byte characters, which count more than once.

### Invalid cross-device link
Means: EXDEV: a hard link or rename can't span two filesystems.
Fix: Use `cp` then `rm` (or `mv`, which does that for you), or a symlink instead of a hard link.

### Too many levels of symbolic links
Means: ELOOP: a symlink points (directly or through others) back to itself, or the chain is longer than 40.
Fix: `namei -l <path>` shows the chain. Fix the link target, using an absolute path if the relative one is wrong.

### Resource temporarily unavailable
Means: EAGAIN: a non-blocking operation would have had to wait, or a fork failed because the user hit their process limit.
Fix: For "fork: Resource temporarily unavailable" check `ulimit -u` and `systemctl show -p TasksMax <unit>`; look for a process leak.

### fork: retry: Resource temporarily unavailable
Means: bash couldn't create a new process: the user or the cgroup reached its process/thread limit (ulimit -u, TasksMax, pids.max) or the system ran out of PIDs.
Fix: `ps -eLf | wc -l` and `ps -u <user> | wc -l`. Find the runaway program; raise `TasksMax=` or the container's `pids_limit` only if the load is legitimate.

### fork: Cannot allocate memory
Means: The kernel refused to create a new process, usually from memory pressure with strict overcommit, or a full PID table.
Fix: `free -h`, `cat /proc/sys/kernel/pid_max`, `ps -e | wc -l`. Free memory or kill the leaking process.

### Cannot allocate memory
Means: ENOMEM: the kernel couldn't give the program the memory it asked for (real exhaustion, a cgroup limit, or `vm.overcommit_memory=2`).
Fix: `free -h`, check limits on the unit or container, and `sysctl vm.overcommit_memory`.

### sudo: <name>: command not found
Means: sudo uses its own secure_path, which may not include the directory where the program lives (for example /usr/local/bin or ~/.local/bin).
Fix: Use the full path (`sudo $(command -v <name>)`), or add the directory to `secure_path` with `visudo`.

### cd: too many arguments
Means: The directory name has spaces and wasn't quoted, so cd got several words.
Fix: Quote it: `cd "/path/with spaces"`.

### cd: <dir>: Permission denied
Means: You lack execute (search) permission on that directory or one of its parents.
Fix: `namei -l <dir>` shows which component blocks you; add `x` for your user or group there.

### event not found
Means: An unquoted or double-quoted `!` triggered bash history expansion (for example a password with `!` in it).
Fix: Use single quotes around the text, or turn expansion off with `set +H`.

### Interrupted system call
Means: EINTR: a blocking call was interrupted by a signal before it finished.
Fix: Usually harmless and retried by the program. If a tool gives up because of it, re-run it; it's a program bug if it happens constantly.

### Input/output error
Means: EIO: the kernel couldn't read or write the underlying device: a failing disk, a bad cable, a dropped USB disk, or a network filesystem that lost its server.
Fix: `dmesg -T | tail -50` for disk or NFS errors, then `smartctl -a /dev/<disk>`. Back up what you can before running repairs.

### Stale file handle
Means: ESTALE: the file or directory you had open on an NFS mount was deleted or the export changed on the server.
Fix: `cd` out and back in; if it persists, unmount and remount (`umount -l` if busy). Don't export filesystems whose IDs change (set `fsid=` on the export).

### Transport endpoint is not connected
Means: ENOTCONN: a FUSE mount (sshfs, rclone, mergerfs, s3fs) died but is still listed as mounted.
Fix: `fusermount -uz <mountpoint>` (or `umount -l`), then mount it again. Check the FUSE process's log for why it died.

### Value too large for defined data type
Means: EOVERFLOW: a 32-bit program met a file larger than 2 GB or an inode number it can't hold (common on large NFS/XFS volumes with old 32-bit tools).
Fix: Use a 64-bit build of the tool, or a modern version built with large file support.

### Function not implemented
Means: ENOSYS: the kernel or filesystem doesn't support that system call (old kernel, a container's seccomp filter, or an emulated platform).
Fix: Check the kernel version (`uname -r`) the program needs; in containers check the seccomp profile and Docker version.

### Operation not supported
Means: EOPNOTSUPP: the filesystem or device doesn't support that operation (for example extended attributes, fallocate, or symlinks on FAT/exFAT/SMB mounts).
Fix: Use a Linux filesystem (ext4, XFS, Btrfs, ZFS) for that data, or a mount option that enables the feature.

### xargs: <cmd>: terminated by signal <n>
Means: One of the commands xargs ran was killed by that signal (9 = SIGKILL, often out of memory).
Fix: Run that command alone on the same input to see its error; check `dmesg` for OOM kills.

### tar: Exiting with failure status due to previous errors
Means: tar finished but hit errors along the way, often unreadable files or files that changed while being read.
Fix: Scroll up for the first "tar:" line. Run as root for permission errors; snapshot first for live data.

### tar: Removing leading `/' from member names
Means: Not an error: tar stores absolute paths as relative so extraction can't overwrite system files.
Fix: Nothing to do. Extract with `-C /` if you really want the original locations.

### gzip: stdin: not in gzip format
Means: The file isn't gzip-compressed (it may be plain tar, xz, zstd, or an HTML error page saved by a failed download).
Fix: `file <archive>`. Use `tar xf` (it detects the format), or re-download if it's HTML.

### unzip: cannot find zipfile directory
Means: The zip file is truncated or isn't a zip at all (often an incomplete download).
Fix: `file <file>` and compare its size to the source; download it again.

### xz: (stdin): File format not recognized
Means: The data isn't xz-compressed.
Fix: `file <file>`; use the matching tool (`zstd -d`, `gunzip`, `bunzip2`).

### sed: -e expression #1, char <n>: unterminated `s' command
Means: The `s/old/new/` expression is missing a delimiter, usually because the text contains `/`.
Fix: Use another delimiter: `sed 's|/old/path|/new/path|'`.

### sed: -i requires an argument
Means: On macOS/BSD, `sed -i` needs a backup suffix argument (GNU sed does not).
Fix: Use `sed -i '' 's/a/b/' file` on macOS, or install GNU sed (`gsed`).

### awk: cmd. line:1: syntax error
Means: The awk program text has a syntax error, often because the shell expanded `$1` inside double quotes.
Fix: Put the awk program in single quotes: `awk '{print $1}'`.

### grep: Unmatched ( or \(
Means: The pattern has an unbalanced parenthesis in basic regex mode.
Fix: Use `grep -E` for extended regex, or `grep -F` for a fixed string, and quote the pattern.

### find: paths must precede expression
Means: An unquoted glob like `-name *.log` expanded to several filenames before find saw it.
Fix: Quote the pattern: `find . -name '*.log'`.

### find: missing argument to `-exec'
Means: `-exec` must end with `\;` or `+`.
Fix: `find . -name '*.tmp' -exec rm {} +`.

### date: invalid date
Means: GNU date couldn't parse the string given to `-d`.
Fix: Use ISO format (`2026-11-17 14:00`) or relative forms (`-d '2 days ago'`). macOS date uses `-v` and `-j -f` instead.

### crontab: installing new crontab "<file>":<n>: bad minute
Means: A crontab line has an invalid value in the minute field (or the line has too few fields).
Fix: Each line needs five time fields then the command. DRIFT's cron check explains any expression you paste.

### /usr/bin/env: 'python': No such file or directory
Means: The script asks env for `python`, but only `python3` is installed (modern distros don't ship a `python` command).
Fix: Change the #! line to `python3`, or install `python-is-python3` on Debian/Ubuntu.

### /usr/bin/env: '<interpreter>\r': No such file or directory
Means: The script has Windows line endings, so env looks for an interpreter name ending in a carriage return.
Fix: `sed -i 's/\r$//' <file>`.

### nohup: ignoring input and appending output to 'nohup.out'
Means: Not an error: nohup redirects output to nohup.out because you didn't.
Fix: Redirect yourself (`nohup cmd > cmd.log 2>&1 &`), or better, run it as a systemd service or in tmux.

### screen: Cannot open your terminal '/dev/pts/<n>' - please check.
Means: After `su` to another user, that user doesn't own your terminal device.
Fix: Log in as that user directly (ssh), or use `script /dev/null` first. tmux avoids this too.

### open terminal failed: missing or unsuitable terminal: <term>
Means: tmux (or another full-screen program) doesn't know your TERM value, often xterm-kitty, alacritty or wezterm on a server without their terminfo.
Fix: `export TERM=xterm-256color`, or install the terminal's terminfo on the server (`infocmp -x | ssh host tic -x -`).

### Error opening terminal: <term>.
Means: ncurses programs (nano, htop, less) can't find terminfo for the TERM your client sent.
Fix: `TERM=xterm-256color <program>`, or install `ncurses-term` (Debian/Ubuntu) for extra terminfo entries.

### WARNING: terminal is not fully functional
Means: less (or man) runs in a terminal it doesn't fully know, usually TERM=dumb or unknown.
Fix: Set TERM to `xterm-256color`; in non-interactive scripts use `--no-pager` or `PAGER=cat`.

### stty: 'standard input': Inappropriate ioctl for device
Means: ENOTTY: a terminal command ran without a terminal (in a script, cron job, or `ssh host cmd`).
Fix: Guard it in your shell rc with `[ -t 0 ] &&`, or use `ssh -t` when you need a terminal.

### mesg: ttyname failed: Inappropriate ioctl for device
Means: Harmless: root's ~/.profile on Debian/Ubuntu runs `mesg n` even when there's no terminal (for example a non-interactive SSH command).
Fix: Change that line to `tty -s && mesg n || true`.

### bash: warning: setlocale: LC_ALL: cannot change locale (<locale>)
Means: The locale your client sends (often via SSH's SendEnv) isn't generated on the server.
Fix: Generate it (`sudo locale-gen <locale>` on Debian/Ubuntu, `dnf install glibc-langpack-<lang>` on Fedora) or stop sending it (remove `SendEnv LANG LC_*` on the client).

### perl: warning: Setting locale failed.
Means: Same cause as the setlocale warning: the LANG/LC_ values from your session aren't installed on the server.
Fix: `sudo locale-gen en_US.UTF-8 && sudo update-locale`, or export a locale that `locale -a` lists.

### -bash: /dev/null: Permission denied
Means: /dev/null lost its normal permissions (someone replaced it with a regular file, or chmod'ed it).
Fix: As root: `rm -f /dev/null && mknod -m 666 /dev/null c 1 3`.

### ulimit: open files: cannot modify limit: Operation not permitted
Means: A normal user can't raise a hard limit, only lower it or raise the soft limit up to the hard one.
Fix: Raise it in /etc/security/limits.d/ (login sessions) or with `LimitNOFILE=` in the systemd unit for services.
