/**
 * Shared formatting for the pre-run cost previews shown next to each AI panel's
 * Run button. Kept locale-neutral on purpose: the "<$0,01" / "$X.XX" shape
 * matches what PolishPanel/SuggestPanel/ClipsPanel/TranslatePanel have always
 * rendered, so the estimate reads the same everywhere.
 */
export function formatCost(usd: number): string {
  if (usd < 0.01) return "<$0,01";
  return `$${usd.toFixed(2)}`;
}
