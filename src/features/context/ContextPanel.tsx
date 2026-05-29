/**
 * Context & glossary — killer feature #2 (Phase 3.4), modes 1 + 2.
 *
 * Mode 1: a free-text description of what the recording is about. Mode 2: a
 * glossary of proper nouns / jargon, each with the misrecognitions to
 * auto-correct. Both feed Whisper's initial_prompt before transcription
 * (priming) and the post-pass correction (applyGlossary) after. AI-suggested
 * glossary entries from a first-pass transcript (mode 3) are a later step.
 *
 * The project is the renderer-side source of truth, so edits here are plain
 * state updates via onProjectChange; the backend pass is invoked on demand.
 */

import { useState } from "react";
import { BookText, Plus, Trash2, Wand2, Sparkles, Loader2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { GlossaryTerm, Project, SuggestedTerm } from "@/lib/bindings";

interface Props {
  project: Project;
  onProjectChange: (p: Project) => void;
}

export function ContextPanel({ project, onProjectChange }: Props) {
  const [applyMsg, setApplyMsg] = useState<string | null>(null);
  const [suggested, setSuggested] = useState<SuggestedTerm[] | null>(null);
  const [suggesting, setSuggesting] = useState(false);
  const [suggestMsg, setSuggestMsg] = useState<string | null>(null);

  function setContext(value: string) {
    onProjectChange({
      ...project,
      context_description: value.trim() === "" ? null : value,
    });
  }

  function updateTerm(id: string, patch: Partial<GlossaryTerm>) {
    onProjectChange({
      ...project,
      glossary: project.glossary.map((t) =>
        t.id === id ? { ...t, ...patch } : t,
      ),
    });
  }

  function addTerm() {
    const term: GlossaryTerm = {
      id: crypto.randomUUID(),
      term: "",
      aliases: [],
      definition: null,
      pronunciation_hint: null,
    };
    onProjectChange({ ...project, glossary: [...project.glossary, term] });
  }

  function removeTerm(id: string) {
    onProjectChange({
      ...project,
      glossary: project.glossary.filter((t) => t.id !== id),
    });
  }

  async function applyNow() {
    setApplyMsg(null);
    try {
      const res = await ipc.ops.applyGlossary(project);
      onProjectChange(res.project);
      setApplyMsg(
        res.corrections.length === 0
          ? "Ingen termer å rette."
          : `Rettet ${res.corrections.length} forekomst(er).`,
      );
    } catch (e) {
      setApplyMsg(e instanceof IPCError ? `Feil: ${e.message}` : String(e));
    }
  }

  // Killer feature #2, mode 3: ask the LLM to propose glossary terms from the
  // transcript. Propose-and-approve — nothing is added until the user accepts.
  async function runSuggest() {
    setSuggesting(true);
    setSuggestMsg(null);
    setSuggested(null);
    try {
      const out = await ipc.glossary.suggest(project, "haiku45");
      setSuggested(out);
      if (out.length === 0) setSuggestMsg("Fant ingen nye termer å foreslå.");
    } catch (e) {
      // Most likely: no Anthropic key set.
      setSuggestMsg(
        e instanceof IPCError
          ? `Feil: ${e.message} (legg inn Anthropic-nøkkel i Innstillinger?)`
          : String(e),
      );
    } finally {
      setSuggesting(false);
    }
  }

  function acceptSuggestion(s: SuggestedTerm) {
    const exists = project.glossary.some(
      (t) => t.term.toLowerCase() === s.term.toLowerCase(),
    );
    if (!exists) {
      const term: GlossaryTerm = {
        id: crypto.randomUUID(),
        term: s.term,
        aliases: s.aliases,
        definition: s.reason.trim() || null,
        pronunciation_hint: null,
      };
      onProjectChange({ ...project, glossary: [...project.glossary, term] });
    }
    setSuggested((cur) => cur?.filter((t) => t !== s) ?? null);
  }

  function dismissSuggestion(s: SuggestedTerm) {
    setSuggested((cur) => cur?.filter((t) => t !== s) ?? null);
  }

  return (
    <div className="mx-auto max-w-2xl p-6">
      <div className="mb-1 flex items-center gap-2">
        <BookText size={18} className="text-[var(--color-accent-400)]" />
        <h2 className="text-[var(--text-ui-lg)] font-semibold">
          Kontekst og ordliste
        </h2>
      </div>
      <p className="mb-6 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        Fortell SundayEdit hva opptaket handler om og hvilke navn/fagord som
        forekommer. Det gjør gjenkjenningen mer nøyaktig (priming) og retter
        kjente feilstavinger automatisk.
      </p>

      {/* Mode 1 — free-text context */}
      <label className="mb-2 block text-[var(--text-ui-sm)] font-medium">
        Beskrivelse
      </label>
      <textarea
        value={project.context_description ?? ""}
        onChange={(e) => setContext(e.target.value)}
        rows={3}
        placeholder="F.eks. «En preken om kristologi og soteriologi. Taleren er norsk.»"
        className="w-full resize-y rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-2 text-[var(--text-ui-sm)] outline-none focus:border-[var(--color-accent-500)]"
      />

      {/* Mode 2 — glossary */}
      <div className="mb-2 mt-6 flex items-center justify-between">
        <label className="text-[var(--text-ui-sm)] font-medium">
          Ordliste ({project.glossary.length})
        </label>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={runSuggest}
            disabled={suggesting}
            title="La AI foreslå termer fra transkripsjonen"
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-[var(--text-ui-xs)] font-medium hover:border-[var(--color-accent-600)] disabled:opacity-50"
          >
            {suggesting ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Sparkles size={12} />
            )}
            Foreslå termer (AI)
          </button>
          <button
            type="button"
            onClick={addTerm}
            className="flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-[var(--text-ui-xs)] font-medium hover:border-[var(--color-accent-600)]"
          >
            <Plus size={12} /> Legg til term
          </button>
        </div>
      </div>

      {/* AI suggestion review queue — accept adds to the glossary above. */}
      {suggestMsg && (
        <p className="mb-3 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
          {suggestMsg}
        </p>
      )}
      {suggested && suggested.length > 0 && (
        <div className="mb-4 rounded-lg border border-[var(--color-accent-600)]/40 bg-[var(--color-accent-500)]/5 p-3">
          <p className="mb-2 flex items-center gap-1.5 text-[var(--text-ui-xs)] font-semibold text-[var(--color-accent-300)]">
            <Sparkles size={12} /> {suggested.length} forslag — godta dem du vil
            ha
          </p>
          <ul className="space-y-2">
            {suggested.map((s, i) => (
              <li
                key={`${s.term}-${i}`}
                className="flex items-start gap-2 rounded-md bg-[var(--color-bg-elevated)] px-3 py-2"
              >
                <div className="flex-1">
                  <span className="font-mono text-[var(--text-ui-sm)] font-semibold">
                    {s.term}
                  </span>
                  {s.aliases.length > 0 && (
                    <span className="ml-2 text-[10px] text-[var(--color-fg-subtle)]">
                      ↔ {s.aliases.join(", ")}
                    </span>
                  )}
                  {s.reason && (
                    <p className="mt-0.5 text-[10px] text-[var(--color-fg-muted)]">
                      {s.reason}
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  onClick={() => acceptSuggestion(s)}
                  className="rounded-md bg-[var(--color-accent-600)] px-2.5 py-1 text-[10px] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
                >
                  Legg til
                </button>
                <button
                  type="button"
                  onClick={() => dismissSuggestion(s)}
                  title="Forkast"
                  aria-label="Forkast forslag"
                  className="rounded-md p-1 text-[var(--color-fg-subtle)] hover:text-[var(--color-fg)]"
                >
                  <Trash2 size={13} />
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {project.glossary.length === 0 ? (
        <p className="rounded-lg border border-dashed border-[var(--color-border)] px-3 py-6 text-center text-[var(--text-ui-sm)] text-[var(--color-fg-subtle)]">
          Ingen termer ennå. Legg til navn, fagord eller fremmedord som Whisper
          bør forvente.
        </p>
      ) : (
        <ul className="space-y-3">
          {project.glossary.map((t) => (
            <li
              key={t.id}
              className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-3"
            >
              <div className="flex items-start gap-2">
                <div className="grid flex-1 grid-cols-2 gap-2">
                  <Field
                    label="Term (riktig form)"
                    value={t.term}
                    onChange={(v) => updateTerm(t.id, { term: v })}
                    placeholder="kerygma"
                  />
                  <Field
                    label="Feilstavinger (komma)"
                    value={t.aliases.join(", ")}
                    onChange={(v) =>
                      updateTerm(t.id, {
                        aliases: v
                          .split(",")
                          .map((a) => a.trim())
                          .filter(Boolean),
                      })
                    }
                    placeholder="kerigma, kerygmae"
                  />
                  <Field
                    label="Definisjon (valgfritt)"
                    value={t.definition ?? ""}
                    onChange={(v) =>
                      updateTerm(t.id, { definition: v.trim() || null })
                    }
                    placeholder="forkynnelsen av evangeliet"
                  />
                  <Field
                    label="Uttale (valgfritt)"
                    value={t.pronunciation_hint ?? ""}
                    onChange={(v) =>
                      updateTerm(t.id, {
                        pronunciation_hint: v.trim() || null,
                      })
                    }
                    placeholder="ke-ROOG-ma"
                  />
                </div>
                <button
                  type="button"
                  onClick={() => removeTerm(t.id)}
                  title="Fjern term"
                  aria-label="Fjern term"
                  className="mt-5 shrink-0 rounded-md p-1.5 text-[var(--color-fg-subtle)] hover:bg-[var(--color-danger)]/10 hover:text-[var(--color-danger)]"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-6 flex items-center gap-3">
        <button
          type="button"
          onClick={applyNow}
          disabled={project.glossary.length === 0}
          className="flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-4 py-2 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-50"
        >
          <Wand2 size={14} /> Rett termer på undertekstene nå
        </button>
        {applyMsg && (
          <span className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            {applyMsg}
          </span>
        )}
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[10px] uppercase tracking-wider text-[var(--color-fg-subtle)]">
        {label}
      </span>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 text-[var(--text-ui-sm)] outline-none focus:border-[var(--color-accent-500)]"
      />
    </label>
  );
}
