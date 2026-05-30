/**
 * Sunday-link deep-link import — Phase 8 (renderer side).
 *
 * The native layer hands us a validated `ImportRequest` (parsed in Rust) after
 * a sister Sunday-suite app launches us with `sundayedit://import?…`. We then
 * create a project from the video the normal way and fold the carried context
 * into it: transcription language, the priming description, and any glossary
 * terms (speaker names, jargon) the caller supplied. This keeps the killer
 * context feature filled in before the first transcription.
 *
 * Pure and id-injectable so it's testable without a Tauri runtime.
 */
import type { GlossaryTerm, ImportRequest, Project } from "@/lib/bindings";

/**
 * Return a copy of `project` seeded with the language, context, and glossary
 * carried by a deep-link `ImportRequest`. Glossary terms already present
 * (case-insensitive on `term`) are not duplicated; the request's own list is
 * already de-duped in Rust. `makeId` is injectable for deterministic tests.
 */
export function seedProjectFromImport(
  project: Project,
  req: ImportRequest,
  makeId: () => string = () => crypto.randomUUID(),
): Project {
  const existing = new Set(
    project.glossary.map((g) => g.term.trim().toLowerCase()),
  );
  const added: GlossaryTerm[] = [];
  for (const name of req.glossary) {
    const trimmed = name.trim();
    const key = trimmed.toLowerCase();
    if (!key || existing.has(key)) continue;
    existing.add(key);
    added.push({
      id: makeId(),
      term: trimmed,
      aliases: [],
      definition: null,
      pronunciation_hint: null,
    });
  }

  return {
    ...project,
    language: req.language ?? project.language,
    context_description: req.context ?? project.context_description,
    glossary: [...project.glossary, ...added],
  };
}
