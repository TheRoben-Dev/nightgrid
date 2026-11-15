import { describe, expect, it } from "vitest";
import { composeYaml, envText, formFromImage, formProblems, nameFromImage, yamlStr } from "./composeGen";

const info = {
  image: "ghcr.io/acme/web-app:2",
  version: { version: "2.0.0", revision: null, created: null },
  ports: ["8080/tcp", "53/udp"],
  volumes: ["/config"],
  env: ["TZ", "ADMIN_TOKEN"],
  source: null,
  description: null,
};

describe("composeGen", () => {
  it("builds a compose file and keeps env values out of it", () => {
    const f = formFromImage(info);
    expect(f.name).toBe("web-app");
    expect(nameFromImage("nginx:1.27")).toBe("nginx");
    f.env[1].value = "s3cr3t value";
    const y = composeYaml(f);
    expect(y).toContain("image: ghcr.io/acme/web-app:2");
    expect(y).toContain('- "8080:8080"');
    expect(y).toContain('- "53:53/udp"');
    expect(y).toContain('- "./config:/config"');
    expect(y).toContain("ADMIN_TOKEN: ${ADMIN_TOKEN}");
    expect(y).not.toContain("s3cr3t");
    expect(envText(f)).toBe("TZ=\nADMIN_TOKEN='s3cr3t value'\n");
    expect(formProblems(f)).toEqual([]);
  });

  it("flags problems and quotes YAML", () => {
    const f = formFromImage(info);
    f.ports.push({ host: "8080", container: "80", proto: "tcp" });
    f.ports.push({ host: "x", container: "1", proto: "tcp" });
    f.env.push({ name: "1BAD", value: "" });
    const p = formProblems(f);
    expect(p.some((x) => x.includes("used twice"))).toBe(true);
    expect(p.some((x) => x.includes("valid port"))).toBe(true);
    expect(p.some((x) => x.includes("1BAD"))).toBe(true);
    expect(yamlStr("yes")).toBe('"yes"');
    expect(yamlStr("nginx")).toBe("nginx");
    expect(yamlStr('a"b:c')).toBe('"a\\"b:c"');
  });
});
