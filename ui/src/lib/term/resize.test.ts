import { afterEach, describe, expect, it, vi } from "vitest";

let release: (id: number) => void = () => {};
const termResize = vi.fn((_id: number, _c: number, _r: number) => Promise.resolve(null));
const termOpenLocal = vi.fn((_c: number, _r: number) => new Promise<number>((r) => (release = r)));
vi.mock("../api", async (orig) => {
  const real = await orig<typeof import("../api")>();
  return { ...real, api: { ...real.api, termResize, termOpenLocal, termClose: vi.fn(() => Promise.resolve(null)) } };
});

const { TermSession } = await import("./session.svelte");

describe("terminal size reaches the backend", () => {
  afterEach(() => {
    vi.useRealTimers();
    termResize.mockClear();
  });

  it("a resize while connecting (e.g. a split) is sent once the session opens", async () => {
    vi.useFakeTimers();
    const s = new TermSession(1, { kind: "local" });
    const opening = s.start();
    expect(termOpenLocal).toHaveBeenCalledWith(80, 24, expect.anything());
    s.term.resize(57, 30); // the pane got narrower before the shell answered
    vi.advanceTimersByTime(100);
    expect(termResize).not.toHaveBeenCalled();
    release(7);
    await opening;
    expect(termResize).toHaveBeenLastCalledWith(7, 57, 30);
    s.dispose();
  });

  it("rapid split/close/resize sends only the final size", async () => {
    vi.useFakeTimers();
    const s = new TermSession(2, { kind: "local" });
    const opening = s.start();
    release(8);
    await opening;
    termResize.mockClear();
    for (let i = 0; i < 40; i++) s.term.resize(40 + (i % 7) * 10, 10 + (i % 5) * 4);
    s.term.resize(101, 33);
    vi.advanceTimersByTime(100);
    expect(termResize).toHaveBeenCalledTimes(1);
    expect(termResize).toHaveBeenLastCalledWith(8, 101, 33);
    // Same size again: nothing new to tell the backend.
    s.term.resize(101, 33);
    vi.advanceTimersByTime(100);
    expect(termResize).toHaveBeenCalledTimes(1);
    s.dispose();
  });
});
