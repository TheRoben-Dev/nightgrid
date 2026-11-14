import { describe, expect, it } from "vitest";
import { api } from "../api";
import { mockSample } from "../mockMetrics";
import { formatRate, formatUptime, metrics, SPARK_POINTS } from "./metrics.svelte";

describe("metrics store", () => {
  it("tracks state events and samples per host", () => {
    expect(metrics.get(901).state).toBe("connecting");
    metrics.onEvent({ kind: "state", hostId: 901, state: "offline", message: "timed out", retrySecs: 20 });
    expect(metrics.get(901)).toMatchObject({ state: "offline", message: "timed out", retrySecs: 20 });

    const s = mockSample(1, 3);
    metrics.onEvent({ kind: "sample", hostId: 901, sample: s });
    const m = metrics.get(901);
    expect(m.state).toBe("ok");
    expect(m.message).toBeNull();
    expect(m.sample?.hostname).toBe(s.hostname);
    expect(m.cpu).toEqual(s.cpu == null ? [] : [s.cpu]);
    expect(m.net).toEqual([s.netRx + s.netTx]);
  });

  it("caps sparkline history and skips missing values", () => {
    for (let t = 0; t < SPARK_POINTS + 25; t++) {
      metrics.onEvent({ kind: "sample", hostId: 902, sample: { ...mockSample(2, t), cpuTemp: null } });
    }
    const m = metrics.get(902);
    expect(m.cpu).toHaveLength(SPARK_POINTS);
    expect(m.mem).toHaveLength(SPARK_POINTS);
    expect(m.temp).toHaveLength(0);
    expect(m.cpu.at(-1)).toBe(mockSample(2, SPARK_POINTS + 24).cpu);
  });

  it("merges visible hosts from several owners", async () => {
    const seen: number[][] = [];
    const orig = api.metricsSetVisible;
    api.metricsSetVisible = async (ids: number[]) => {
      seen.push([...ids].sort());
      return null;
    };
    try {
      metrics.setVisible("overview", [1, 2]);
      metrics.setVisible("monitor", [2, 3]);
      metrics.setVisible("overview", []);
      expect(seen).toEqual([[1, 2], [1, 2, 3], [2, 3]]);
      metrics.setVisible("monitor", []);
    } finally {
      api.metricsSetVisible = orig;
    }
  });

  it("formats rates and uptime", () => {
    expect(formatRate(0)).toBe("0 B/s");
    expect(formatRate(512)).toBe("512 B/s");
    expect(formatRate(1536)).toBe("1.5 KB/s");
    expect(formatRate(250 * 1024 * 1024)).toBe("250 MB/s");
    expect(formatUptime(59)).toBe("0m");
    expect(formatUptime(3700)).toBe("1h 1m");
    expect(formatUptime(93784)).toBe("1d 2h");
  });
});
