import { describe, expect, it } from "vitest";

import type { ImportRequest, Project } from "@/lib/bindings";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";

import { seedProjectFromImport } from "./deepLinkImport";

// A small, known base so assertions don't ride on the sample's exact content.
const base: Project = {
  ...SAMPLE_PROJECT,
  language: "en",
  context_description: null,
  glossary: [
    {
      id: "g1",
      term: "kerygma",
      aliases: [],
      definition: null,
      pronunciation_hint: null,
    },
  ],
};

let n = 0;
const counter = () => `id-${++n}`;

function req(partial: Partial<ImportRequest>): ImportRequest {
  return {
    path: "/v.mp4",
    language: null,
    context: null,
    glossary: [],
    return_to: null,
    ...partial,
  };
}

describe("seedProjectFromImport", () => {
  it("applies language and context from the request", () => {
    const out = seedProjectFromImport(
      base,
      req({ language: "no", context: "Sermon, speaker: Ola" }),
      counter,
    );
    expect(out.language).toBe("no");
    expect(out.context_description).toBe("Sermon, speaker: Ola");
  });

  it("keeps the project's own values when the request omits them", () => {
    const out = seedProjectFromImport(base, req({}), counter);
    expect(out.language).toBe("en");
    expect(out.context_description).toBeNull();
  });

  it("appends new glossary terms with generated ids", () => {
    n = 0;
    const out = seedProjectFromImport(
      base,
      req({ glossary: ["Ola Nordmann", "Babbage"] }),
      counter,
    );
    expect(out.glossary.map((g) => g.term)).toEqual([
      "kerygma",
      "Ola Nordmann",
      "Babbage",
    ]);
    expect(out.glossary.slice(1).map((g) => g.id)).toEqual(["id-1", "id-2"]);
  });

  it("does not duplicate a term already in the glossary (case-insensitive)", () => {
    const out = seedProjectFromImport(
      base,
      req({ glossary: ["KERYGMA", "Newton"] }),
      counter,
    );
    expect(out.glossary.map((g) => g.term)).toEqual(["kerygma", "Newton"]);
  });

  it("does not mutate the input project", () => {
    const before = structuredClone(base);
    seedProjectFromImport(
      base,
      req({ glossary: ["X"], language: "de" }),
      counter,
    );
    expect(base).toEqual(before);
  });
});
