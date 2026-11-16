import { describe, expect, it } from "vitest";
import { checkCommand, looksLikeCommand, segments, worst } from "./command";
import { parseYaml, reviewCompose } from "./compose";
import { cronFromWords, describeCron, findCron, nextRuns, parseCron, toOnCalendar } from "./cron";
import { exitCode, lookupErrors } from "./errors";
import { checksText, commandsIn, runSkills } from "./index";

const whats = (cmd: string) => checkCommand(cmd).map((f) => `${f.level}:${f.what}`);

describe("command check", () => {
  it("splits on pipes and separators, keeping quotes", () => {
    expect(segments(`echo "a | b" | grep a && rm -f 'x y'; ls`).map((s) => s.words)).toEqual([["echo", "a | b"], ["grep", "a"], ["rm", "-f", "x y"], ["ls"]]);
    expect(segments("curl -fsSL x | sh")[1].pipedFrom).toBe(true);
  });

  it("flags the classic dangers", () => {
    expect(whats("sudo rm -rf /")).toContain("danger:rm -r /");
    expect(whats("rm -rf ./build")).toContain("danger:rm -rf");
    expect(whats("rm -r --force $DIR/")).toEqual(expect.arrayContaining(["danger:rm -rf", "danger:$DIR/"]));
    expect(whats("chmod -R 777 /var/www")).toContain("danger:chmod 777");
    expect(whats("curl -fsSL https://example.com/install.sh | sudo bash")).toContain("danger:| bash");
    expect(whats("dd if=img.iso of=/dev/sdb bs=4M")).toContain("danger:of=/dev/sdb");
    expect(whats("mkfs.ext4 /dev/sdb1")).toContain("danger:mkfs.ext4");
    expect(whats("docker system prune -a --volumes")).toEqual(expect.arrayContaining(["danger:docker system prune -a", "danger:--volumes"]));
    expect(whats("docker compose down -v")).toContain("danger:compose down -v");
    expect(whats("git push -f origin main")).toContain("danger:git push --force");
    expect(whats("sudo systemctl stop sshd")).toContain("danger:systemctl stop sshd");
    expect(whats("iptables -F")).toContain("danger:iptables -F");
    expect(whats("crontab -r")).toContain("danger:crontab -r");
    expect(whats("echo x > /dev/sda")).toContain("danger:> /dev/sda");
    expect(whats(":(){ :|:& };:")[0]).toBe("danger::(){ :|:& };:");
    expect(whats("find /tmp -name '*.log' -delete")).toContain("danger:find -delete");
  });

  it("is calm about safe commands and notes cautions", () => {
    expect(checkCommand("ls -la /etc")).toEqual([]);
    expect(checkCommand("docker ps -a")).toEqual([]);
    expect(checkCommand("journalctl -u nginx 2>&1 > /dev/null")).toEqual([]);
    expect(worst(checkCommand("sudo apt upgrade -y"))).toBe("caution");
    expect(whats("sed -i 's/a/b/' file")).toContain("caution:sed -i");
    expect(whats("echo hi > notes.txt")).toContain("info:> notes.txt");
    expect(whats("$ sudo reboot")).toContain("danger:reboot");
  });

  it("recognizes commands", () => {
    expect(looksLikeCommand("rm -rf /tmp/x")).toBe(true);
    expect(looksLikeCommand("$ docker compose up -d")).toBe(true);
    expect(looksLikeCommand("how do I add a host")).toBe(false);
    expect(commandsIn("what does `chmod 777 x` do?", null)).toEqual(["chmod 777 x"]);
    expect(commandsIn("is it safe to run rm -rf ~/old?", null)).toEqual(["rm -rf ~/old"]);
    expect(commandsIn("explain this", "sudo ufw enable")).toEqual(["sudo ufw enable"]);
  });
});

const COMPOSE = `
version: "3.8"
services:
  db:
    image: postgres
    environment:
      POSTGRES_PASSWORD: hunter2   # oops
      POSTGRES_USER: app
    ports:
      - "5432:5432"
  web:
    image: ghcr.io/example/web:1.4.2
    restart: unless-stopped
    ports:
      - 127.0.0.1:8080:80
      - target: 443
        published: 8443
    environment:
      - API_TOKEN=\${API_TOKEN}
      - LOG_LEVEL=info
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./data:/data
  tool:
    build: .
    privileged: true
    restart: "no"
    command: |
      echo one
      echo two
`;

describe("compose review", () => {
  it("reads the compose subset of YAML", () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const y = parseYaml(COMPOSE) as Record<string, any>;
    expect(Object.keys(y.services)).toEqual(["db", "web", "tool"]);
    expect(y.services.db.environment.POSTGRES_PASSWORD).toBe("hunter2");
    expect(y.services.web.ports).toEqual(["127.0.0.1:8080:80", { target: "443", published: "8443" }]);
    expect(y.services.tool.privileged).toBe("true");
    expect(parseYaml("services:\n  a:\n    ports: [\"80:80\", '443:443']\n")).toEqual({ services: { a: { ports: ["80:80", "443:443"] } } });
  });

  it("flags what matters", () => {
    const r = reviewCompose(COMPOSE);
    const got = r.findings.map((f) => `${f.level}:${f.service ?? "-"}:${f.what}`);
    expect(got).toEqual(
      expect.arrayContaining([
        "info:-:version:",
        "caution:db:no restart policy",
        "caution:db:postgres (no tag)",
        "caution:db:POSTGRES_PASSWORD=…",
        "caution:db:5432:5432",
        "info:web:8443:443",
        "caution:web:docker.sock",
        "danger:tool:privileged: true",
        "info:tool:restart: no",
      ]),
    );
    expect(got.some((g) => g.includes("API_TOKEN"))).toBe(false);
    expect(got.some((g) => g.includes("127.0.0.1:8080"))).toBe(false);
    expect(got.some((g) => g.startsWith("caution:web:no restart"))).toBe(false);
    expect(r.findings[0].level).toBe("danger");
    expect(reviewCompose("hello: world").error).toBeTruthy();
  });
});

describe("error reference", () => {
  it("explains exit codes and systemd results", () => {
    expect(exitCode(137)?.title).toContain("SIGKILL");
    expect(exitCode(127)?.meaning).toContain("not found");
    expect(lookupErrors("Main process exited, code=exited, status=203/EXEC").map((e) => e.id)).toEqual(["systemd-203"]);
    expect(lookupErrors("container exited with code 137").map((e) => e.id)).toContain("exit-137");
    expect(lookupErrors("Main process exited, code=killed, status=9/KILL").map((e) => e.id)).toContain("exit-137");
  });

  it("matches Docker, SSH and apt errors", () => {
    expect(lookupErrors("Error: Bind for 0.0.0.0:80 failed: port is already allocated")[0].id).toBe("port-allocated");
    expect(lookupErrors("me@host: Permission denied (publickey).").map((e) => e.id)).toEqual(["ssh-publickey"]);
    expect(lookupErrors("E: Could not get lock /var/lib/dpkg/lock-frontend")[0].id).toBe("dpkg-lock");
    expect(lookupErrors("write /var/lib/docker/x: no space left on device")[0].id).toBe("no-space");
    expect(lookupErrors("how do I add a host?")).toEqual([]);
  });
});

describe("cron", () => {
  it("describes expressions", () => {
    expect(describeCron("30 3 * * 1-5")).toBe("At 03:30, on Monday to Friday.");
    expect(describeCron("*/15 * * * *")).toBe("Every 15 minutes.");
    expect(describeCron("0 * * * *")).toBe("Every hour, on the hour.");
    expect(describeCron("0 4 1 * *")).toBe("At 04:00, on the 1st of the month.");
    expect(describeCron("@daily")).toBe("At 00:00.");
    expect(describeCron("0 9 * jan,jul sun")).toBe("At 09:00, on Sunday, in January and July.");
    expect(describeCron("@reboot")).toContain("starts");
    expect(describeCron("61 * * * *")).toBeNull();
    expect(parseCron("0 0 * * 7")?.dow.values).toEqual([0]);
  });

  it("computes the next runs", () => {
    const from = new Date(2026, 10, 16, 10, 0); // Monday
    expect(nextRuns("30 3 * * 1-5", from, 2).map((d) => [d.getDate(), d.getHours(), d.getMinutes()])).toEqual([
      [17, 3, 30],
      [18, 3, 30],
    ]);
    expect(nextRuns("0 0 29 2 *", from, 1)[0].getFullYear()).toBe(2028);
  });

  it("turns words into cron and OnCalendar", () => {
    expect(cronFromWords("every 5 minutes")?.expr).toBe("*/5 * * * *");
    expect(cronFromWords("weekdays at 6pm")?.expr).toBe("0 18 * * 1-5");
    expect(cronFromWords("every sunday at 4:30am")?.expr).toBe("30 4 * * 0");
    expect(cronFromWords("daily at 3")?.expr).toBe("0 3 * * *");
    expect(cronFromWords("on the 15th of every month at noon")?.expr).toBe("0 12 15 * *");
    expect(cronFromWords("at boot")?.expr).toBe("@reboot");
    expect(cronFromWords("every 7 minutes")?.note).toBeTruthy();
    expect(cronFromWords("hello there")).toBeNull();
    expect(toOnCalendar("30 3 * * 1-5")).toBe("Mon..Fri *-*-* 03:30:00");
    expect(toOnCalendar("*/15 * * * *")).toBe("*-*-* *:00/15:00");
    expect(findCron("what does 0 */6 * * * mean")).toBe("0 */6 * * *");
    expect(findCron("I have 3 cats and 2 dogs")).toBeNull();
  });
});

describe("skills", () => {
  it("runs the checks that fit and formats them", () => {
    const now = new Date(2026, 10, 16, 10, 0);
    const c = runSkills("is it safe to run `rm -rf $DIR/` ?", null, now);
    expect(c.map((x) => x.kind)).toEqual(["command"]);
    expect(checksText(c)).toContain("[DANGER]");
    expect(runSkills("review this", COMPOSE, now)[0].kind).toBe("compose");
    expect(runSkills("why did it exit with code 137?", null, now).map((x) => x.kind)).toEqual(["errors"]);
    expect(runSkills("make a cron schedule for weekdays at 6pm", null, now)[0].items[0].text).toContain("0 18 * * 1-5");
    expect(runSkills("explain 30 3 * * 1-5", null, now)[0].items.map((i) => i.text).join(" ")).toContain("OnCalendar=Mon..Fri");
    expect(runSkills("How do I add a host?", null, now)).toEqual([]);
  });
});
