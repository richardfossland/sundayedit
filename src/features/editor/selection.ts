/**
 * Pure multi-selection model for the caption list — no React, no rendering,
 * fully unit-testable.
 *
 * A selection is an immutable `Set<string>` of caption ids. All helpers take
 * the current selection plus the project's *ordered* id list (the same order
 * the captions render in) and return a new selection. The ordered list is what
 * makes shift-click range selection and the merge-contiguity check meaningful.
 *
 * The contiguity check mirrors the Rust backend's `merge_captions` rule
 * (services/operations.rs): captions can only merge when their indices in the
 * project's ordering are consecutive. We replicate it here so the bulk bar can
 * disable Merge *before* a doomed round-trip, with the same semantics.
 */

export type Selection = ReadonlySet<string>;

export const EMPTY_SELECTION: Selection = new Set<string>();

/** Toggle a single id in/out of the selection. */
export function toggle(selection: Selection, id: string): Selection {
  const next = new Set(selection);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Is this id currently selected? */
export function isSelected(selection: Selection, id: string): boolean {
  return selection.has(id);
}

/** Clear the whole selection. */
export function clear(): Selection {
  return EMPTY_SELECTION;
}

/** Select every caption in the ordered list. */
export function selectAll(orderedIds: readonly string[]): Selection {
  return new Set(orderedIds);
}

/**
 * Shift-click range select: select the inclusive range between `anchorId` and
 * `targetId` in the ordered list, unioned onto the existing selection. If
 * either id is absent (or there is no anchor), fall back to toggling the
 * target so a stray shift-click never wipes intent.
 */
export function selectRange(
  selection: Selection,
  orderedIds: readonly string[],
  anchorId: string | null,
  targetId: string,
): Selection {
  if (anchorId === null) return toggle(selection, targetId);
  const a = orderedIds.indexOf(anchorId);
  const b = orderedIds.indexOf(targetId);
  if (a === -1 || b === -1) return toggle(selection, targetId);
  const [lo, hi] = a <= b ? [a, b] : [b, a];
  const next = new Set(selection);
  for (let i = lo; i <= hi; i++) next.add(orderedIds[i]);
  return next;
}

/** Number of selected captions. */
export function count(selection: Selection): number {
  return selection.size;
}

/** Selected ids in the project's render order (stable, deterministic). */
export function orderedSelection(
  selection: Selection,
  orderedIds: readonly string[],
): string[] {
  return orderedIds.filter((id) => selection.has(id));
}

/**
 * True when ≥2 captions are selected AND their positions in the ordered list
 * are consecutive — i.e. the backend's `merge_captions` would accept them.
 * Mirrors services/operations.rs so we can disable Merge ahead of the call.
 */
export function isContiguous(
  selection: Selection,
  orderedIds: readonly string[],
): boolean {
  if (selection.size < 2) return false;
  const indices = orderedIds
    .map((id, i) => (selection.has(id) ? i : -1))
    .filter((i) => i !== -1);
  for (let i = 1; i < indices.length; i++) {
    if (indices[i] !== indices[i - 1] + 1) return false;
  }
  return true;
}
