import { describe, it, expect } from "vitest";
import { buildAsrOptions } from "./asrOptions";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import type { GlossaryTerm, Project } from "@/lib/bindings";

function term(over: Partial<GlossaryTerm> & { term: string }): GlossaryTerm {
  return {
    id: over.term,
    aliases: [],
    definition: null,
    pronunciation_hint: null,
    ...over,
  };
}

function project(over: Partial<Project>): Project {
  return { ...SAMPLE_PROJECT, ...over };
}

describe("buildAsrOptions", () => {
  it("carries the project language and the default beam size", () => {
    const opts = buildAsrOptions(project({ language: "nb" }));
    expect(opts.language).toBe("nb");
    expect(opts.beam_size).toBe(5);
  });

  it("falls back to auto when the language is blank", () => {
    expect(buildAsrOptions(project({ language: "" })).language).toBe("auto");
  });

  it("primes with canonical terms first, then aliases, deduped", () => {
    const opts = buildAsrOptions(
      project({
        glossary: [
          term({ term: "kerygma", aliases: ["kerigma", "kerygma"] }),
          term({ term: "frelse", aliases: ["kerygma"] }),
        ],
      }),
    );
    expect(opts.priming_terms).toEqual(["kerygma", "kerigma", "frelse"]);
  });

  it("trims context and nulls it when empty", () => {
    expect(
      buildAsrOptions(project({ context_description: "  En preken  " }))
        .context_description,
    ).toBe("En preken");
    expect(
      buildAsrOptions(project({ context_description: "   " }))
        .context_description,
    ).toBeNull();
  });

  it("drops blank glossary entries", () => {
    const opts = buildAsrOptions(
      project({ glossary: [term({ term: "  ", aliases: ["", "  "] })] }),
    );
    expect(opts.priming_terms).toEqual([]);
  });
});
