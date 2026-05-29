/**
 * Build the AsrOptions that steer a local Whisper run from the project's own
 * language + context + glossary. Pure (no I/O) so it's unit-testable.
 *
 * Killer feature #2 lives here: the glossary's canonical terms and their
 * aliases become Whisper `priming_terms`, biasing recognition toward the
 * names/jargon the user told us to expect. The Rust side assembles them into
 * the `initial_prompt` (see services/asr/mod.rs::initial_prompt).
 */

import type { AsrOptions, Project } from "@/lib/bindings";

export function buildAsrOptions(project: Project): AsrOptions {
  // Canonical term first, then its aliases — deduped, blanks dropped.
  const seen = new Set<string>();
  const priming_terms: string[] = [];
  for (const g of project.glossary) {
    for (const t of [g.term, ...g.aliases]) {
      const term = t.trim();
      if (term && !seen.has(term)) {
        seen.add(term);
        priming_terms.push(term);
      }
    }
  }

  const ctx = project.context_description?.trim();

  return {
    language: project.language || "auto",
    beam_size: 5, // mirrors AsrOptions::default() on the Rust side
    priming_terms,
    context_description: ctx ? ctx : null,
  };
}
