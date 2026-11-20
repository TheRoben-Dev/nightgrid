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
