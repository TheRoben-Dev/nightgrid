import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaletteCommand } from "../palette/types";
import CommandPalette from "./CommandPalette.svelte";

const cmds = (run = vi.fn()): PaletteCommand[] => [
  { id: "a", title: "Go to Docker", group: "NAVIGATE", run },
  { id: "b", title: "Theme: ICE", group: "THEME", run },
  { id: "c", title: "Reduce effects", group: "VIEW", run },
];

// jsdom lacks scrollIntoView.
Element.prototype.scrollIntoView ??= () => {};

afterEach(cleanup);

describe("CommandPalette", () => {
  it("renders nothing when closed", () => {
    render(CommandPalette, { open: false, commands: cmds(), onclose: () => {} });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("filters and runs the selected command with Enter", async () => {
    const run = vi.fn();
    const onclose = vi.fn();
    render(CommandPalette, { open: true, commands: cmds(run), onclose });
    expect(screen.getAllByRole("option")).toHaveLength(3);

    const input = screen.getByRole("combobox");
    await fireEvent.input(input, { target: { value: "dock" } });
    await tick();
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(1);
    expect(options[0].textContent).toContain("Docker");

    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onclose).toHaveBeenCalled();
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("arrow keys move the selection and Escape closes", async () => {
    const onclose = vi.fn();
    render(CommandPalette, { open: true, commands: cmds(), onclose });
    const input = screen.getByRole("combobox");
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getAllByRole("option")[1].getAttribute("aria-selected")).toBe("true");
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getAllByRole("option")[2].getAttribute("aria-selected")).toBe("true");
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });

  it("shows an empty state", async () => {
    render(CommandPalette, { open: true, commands: cmds(), onclose: () => {} });
    await fireEvent.input(screen.getByRole("combobox"), { target: { value: "zzzz" } });
    expect(screen.getByText(/NO MATCH/)).toBeTruthy();
  });
});
