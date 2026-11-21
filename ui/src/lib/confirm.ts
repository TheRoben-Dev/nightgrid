/**
 * Type-to-confirm checks (rule 5). Host, group and key names are matched
 * ignoring case and surrounding spaces: labels around the name are shown in
 * uppercase, and host names are case-insensitive anyway. The name itself is
 * shown exactly as stored (wrap it in `.verbatim`).
 */
export function typedMatches(
  typed: string | null | undefined,
  expected: string | null | undefined,
): boolean {
  if (expected == null) return false;
  const want = expected.trim();
  if (!want) return false;
  return (typed ?? "").trim().toLocaleLowerCase() === want.toLocaleLowerCase();
}

/**
 * True for a plain Enter press that should confirm: main Enter or keypad
 * Enter (Windows reports `code: "NumpadEnter"`), not while an IME is still
 * composing (Windows/Linux IMEs send `keyCode 229` for the commit press), not
 * with modifiers, and not auto-repeat.
 */
export function isConfirmEnter(e: KeyboardEvent): boolean {
  if (e.key !== "Enter" && e.code !== "Enter" && e.code !== "NumpadEnter") return false;
  if (e.isComposing || e.keyCode === 229) return false;
  if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || e.repeat) return false;
  return true;
}

/**
 * `use:enterConfirms` on a type-to-confirm input: Enter clicks the confirm
 * button (`data-confirm`) in the same dialog (or `[data-confirm-scope]`), but
 * only while that button is enabled, so Enter never confirms a wrong name.
 * The button's own `disabled` rule stays the single source of truth.
 */
export function enterConfirms(node: HTMLElement) {
  const onKey = (e: KeyboardEvent) => {
    if (!isConfirmEnter(e)) return;
    e.preventDefault();
    e.stopPropagation();
    const scope: ParentNode = node.closest("[data-confirm-scope], dialog") ?? node.ownerDocument;
    const btn = scope.querySelector<HTMLButtonElement>("button[data-confirm]");
    if (btn && !btn.disabled) btn.click();
  };
  node.addEventListener("keydown", onKey);
  return {
    destroy() {
      node.removeEventListener("keydown", onKey);
    },
  };
}
