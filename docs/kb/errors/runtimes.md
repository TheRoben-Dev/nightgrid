# Errors: Python, Node.js, Java and builds

Messages from language runtimes and build tools you meet when running self-hosted apps or scripts on a server.

### error: externally-managed-environment
Means: Your distro (Debian 12+, Ubuntu 23.04+, Fedora) protects the system Python; pip won't install into it.
Fix: Use a virtual environment (`python3 -m venv ~/venv && ~/venv/bin/pip install <pkg>`), pipx for tools, or the distro package (`apt install python3-<pkg>`).

### ModuleNotFoundError: No module named '<module>'
Means: The module isn't installed for the Python that runs the script (often a different venv or interpreter).
Fix: `which python3` vs where you installed it; activate the venv, or `python3 -m pip install <pkg>` with the same interpreter.

### ImportError: cannot import name '<name>' from '<module>'
Means: The installed version of a library doesn't have that name (too old or too new), or a circular import.
Fix: Install the version the app pins (`pip install -r requirements.txt`).

### SyntaxError: invalid syntax
Means: The Python code doesn't parse: often Python 2 code run with Python 3 (`print "x"`), or a newer syntax on an older Python.
Fix: Check `python3 --version` against what the script needs.

### IndentationError: unexpected indent
Means: Mixed or wrong indentation in Python code (often from copy-paste).
Fix: Use spaces consistently; most editors can convert tabs.

### PermissionError: [Errno 13] Permission denied: '<path>'
Means: The Python process can't read or write that path.
Fix: Check ownership; in containers, the app's uid (PUID/PGID) must own its data folder.

### FileNotFoundError: [Errno 2] No such file or directory: '<path>'
Means: The path doesn't exist, often relative to a different working directory than you expect.
Fix: Use absolute paths, or `cd` into the app folder in the unit/cron job.

### OSError: [Errno 98] Address already in use
Means: Another process listens on the port the app wants.
Fix: `ss -ltnp 'sport = :<port>'`.

### ssl.SSLCertVerificationError: [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: <details>
Means: Python couldn't verify a TLS certificate.
Fix: Fix the chain or install the CA; see the TLS errors shelf.

### MemoryError
Means: Python couldn't allocate memory (container limit or real exhaustion).
Fix: Raise the limit or process data in smaller chunks.

### Killed (Python process)
Means: The kernel killed the process, usually OOM.
Fix: `dmesg -T | grep -i oom`.

### pip: error: subprocess-exited-with-error
Means: Building a package from source failed (missing compiler or headers).
Fix: Install build tools (`build-essential`/`gcc`, `python3-dev`, and the library's -dev package); or use a version with prebuilt wheels.

### fatal error: Python.h: No such file or directory
Means: Python's development headers aren't installed.
Fix: `apt install python3-dev` or `dnf install python3-devel`.

### Error: Cannot find module '<module>'
Means: Node.js can't find that package or file: dependencies not installed, or a wrong path.
Fix: `npm ci` (or `npm install`) in the app folder; check the path and case.

### Error: listen EADDRINUSE: address already in use :::<port>
Means: Another process listens on that port.
Fix: Find and stop it, or change the app's port.

### Error: listen EACCES: permission denied 0.0.0.0:<port>
Means: Non-root processes can't bind ports below 1024.
Fix: Use a high port behind a reverse proxy, or grant `CAP_NET_BIND_SERVICE` (AmbientCapabilities in the unit).

### FATAL ERROR: Reached heap limit Allocation failed - JavaScript heap out of memory
Means: Node hit its heap limit (default about 2–4 GB depending on version).
Fix: `NODE_OPTIONS=--max-old-space-size=4096` if the machine has the RAM; otherwise reduce what the build or app does at once.

### npm ERR! code EACCES
Means: npm can't write to a folder (often global installs into a root-owned prefix).
Fix: Don't sudo npm; set a user prefix (`npm config set prefix ~/.npm-global`) or use a Node version manager.

### npm ERR! code ERESOLVE
Means: npm couldn't resolve peer dependency conflicts.
Fix: Use the lockfile (`npm ci`); `--legacy-peer-deps` as a workaround.

### npm ERR! code ENOTFOUND
Means: npm couldn't resolve the registry host (DNS/network/proxy).
Fix: Check network; `npm config get registry`.

### npm ERR! code ETIMEDOUT
Means: The registry didn't answer in time.
Fix: Retry; check proxy settings.

### npm ERR! code ELIFECYCLE
Means: A package's script (build, postinstall) failed; the real error is above.
Fix: Scroll up to the first error.

### node: --openssl-legacy-provider is not allowed in NODE_OPTIONS
Means: That Node version doesn't accept the flag via NODE_OPTIONS.
Fix: Update the app's build tooling instead of using the flag.

### SyntaxError: Unexpected token '?'
Means: The code uses syntax newer than your Node.js version (here `??` or `?.`).
Fix: Install the Node version the app requires (check `engines` in package.json).

### SyntaxError: Cannot use import statement outside a module
Means: ES module syntax in a file Node treats as CommonJS.
Fix: Use the app's start script; set `"type": "module"` only if the project expects it.

### Error: error:0308010C:digital envelope routines::unsupported (Node)
Means: An old webpack build on Node 17+ uses a hash OpenSSL 3 disabled.
Fix: Upgrade the build tools, or use an LTS Node the project supports.

### java.lang.OutOfMemoryError: Java heap space
Means: The JVM's heap (-Xmx) is too small for the workload.
Fix: Raise `-Xmx` (Minecraft servers, Elasticsearch, etc.) within the container's memory limit, leaving room for non-heap memory.

### java.lang.OutOfMemoryError: Metaspace
Means: Class metadata space ran out (many plugins or a leak).
Fix: Raise `-XX:MaxMetaspaceSize`, or restart and look for the leaking plugin.

### Error: Could not find or load main class <class>
Means: The class or jar isn't on the classpath, or the command is mistyped.
Fix: Check the jar path and `-cp`; use `java -jar <file>.jar` for runnable jars.

### UnsupportedClassVersionError: <class> has been compiled by a more recent version of the Java Runtime (class file version <n>), this version of the Java Runtime only recognizes class file versions up to <n>
Means: The app needs a newer Java (class file 61 = Java 17, 65 = Java 21).
Fix: Install the Java version the app requires (Minecraft 1.20.5+ needs Java 21).

### java.net.BindException: Address already in use
Means: The port is taken.
Fix: Find the other process; often a previous instance still running.

### Exception in thread "main" java.lang.NoClassDefFoundError: <class>
Means: A class present at compile time is missing at runtime (incomplete jar or classpath).
Fix: Use the full distribution or the fat jar the project ships.

### make: *** No rule to make target '<target>'. Stop.
Means: The Makefile doesn't define that target (or you're in the wrong folder).
Fix: Check `make help` or the README.

### make: <cmd>: No such file or directory
Means: A tool the build needs isn't installed (gcc, cc, pkg-config).
Fix: Install build-essential (Debian) or "Development Tools" (Fedora).

### configure: error: no acceptable C compiler found in $PATH
Means: No C compiler is installed.
Fix: `apt install build-essential` or `dnf groupinstall "Development Tools"`.

### Package <name> was not found in the pkg-config search path.
Means: A library's development files (.pc) aren't installed.
Fix: Install `<name>-dev` (Debian) or `<name>-devel` (Fedora).

### error: linker `cc` not found
Means: Rust (cargo) needs a C linker that isn't installed.
Fix: Install build-essential / gcc.

### go: go.mod file not found in current directory or any parent directory
Means: Go commands need a module; you're outside the project, or it uses GOPATH mode.
Fix: `cd` into the project, or `go mod init` for new code.

### PHP Fatal error: Allowed memory size of <n> bytes exhausted
Means: A PHP script hit memory_limit (common with Nextcloud's occ and large imports).
Fix: Raise memory_limit in php.ini, or for one run: `php -d memory_limit=1G occ <command>`.

### php: command not found
Means: The PHP CLI isn't installed on the host (apps in containers have their own).
Fix: Install php-cli, or run the command inside the app's container (`docker exec -u www-data <container> php occ ...`).
