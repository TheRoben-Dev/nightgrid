import { describe, expect, it } from "vitest";
import { blankJob, jobFor, unitFor } from "./jobs";
import { mockJobs } from "./mockJobs";

describe("job editor helpers", () => {
  it("maps names to units", () => {
    expect(unitFor("  Nightly sync! ")).toBe("nightgrid-job-nightly-sync");
    expect(unitFor("!!!")).toBe("");
    const list = [{ unit: "nightgrid-job-a", job: { ...blankJob(), name: "A" } }];
    expect(jobFor({ name: "nightgrid-job-a.timer" }, list)?.job.name).toBe("A");
    expect(jobFor({ name: "apt-daily.timer" }, list)).toBeNull();
  });

  it("edits the mock crontab like the backend", async () => {
    let t = await mockJobs.read(null);
    expect(t.jobs.map((j) => j.enabled)).toEqual([true, false, true]);
    t = await mockJobs.apply(null, t.hash, { op: "setEnabled", line: 4, enabled: true });
    expect(t.jobs[1].enabled).toBe(true);
    await expect(mockJobs.apply(null, "stale", { op: "remove", line: 3 })).rejects.toThrow(/changed/);
    await expect(mockJobs.apply(null, t.hash, { op: "add", schedule: "* * * * *", command: "date +%F" })).rejects.toThrow(/\\%/);
    t = await mockJobs.apply(null, t.hash, { op: "add", schedule: "@daily", command: "true" });
    expect(t.jobs.at(-1)?.command).toBe("true");
  });
});
