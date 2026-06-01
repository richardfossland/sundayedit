/**
 * Unit tests for export-config logic.
 *
 * These are pure-logic tests — no Tauri IPC, no DOM. They verify:
 *   - ExportConfig default values
 *   - patch helpers applied by ExportConfigPanel
 *   - ProjectMeta default values
 */

import { describe, it, expect } from "vitest";
import type { ExportConfig, ProjectMeta, Project } from "@/lib/bindings";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";

// ── Default shapes ──────────────────────────────────────────────────────────

const DEFAULT_EXPORT_CONFIG: ExportConfig = {
  format: "srt",
  burn_in: false,
  caption_size_px: 24,
  caption_color: "white",
  caption_background: "semitransparent",
  max_chars_per_line: 42,
};

const DEFAULT_PROJECT_META: ProjectMeta = {
  title: "",
  description: "",
  proper_nouns: "",
  language: "auto",
};

// Helper that mirrors the patch function in ExportConfigPanel
function patchConfig(project: Project, delta: Partial<ExportConfig>): Project {
  return {
    ...project,
    export_config: { ...project.export_config, ...delta },
  };
}

// Helper that mirrors the patch function in ProjectMetaPanel
function patchMeta(project: Project, delta: Partial<ProjectMeta>): Project {
  return {
    ...project,
    project_meta: { ...project.project_meta, ...delta },
  };
}

describe("ExportConfig defaults", () => {
  it("sample project has sane default export config", () => {
    expect(SAMPLE_PROJECT.export_config).toEqual(DEFAULT_EXPORT_CONFIG);
  });

  it("sample project has sane default project meta", () => {
    expect(SAMPLE_PROJECT.project_meta).toEqual(DEFAULT_PROJECT_META);
  });
});

describe("ExportConfig patch helper", () => {
  it("changes format without touching other fields", () => {
    const p = patchConfig(SAMPLE_PROJECT, { format: "vtt" });
    expect(p.export_config.format).toBe("vtt");
    expect(p.export_config.burn_in).toBe(false);
    expect(p.export_config.caption_size_px).toBe(24);
  });

  it("enables burn_in", () => {
    const p = patchConfig(SAMPLE_PROJECT, { burn_in: true });
    expect(p.export_config.burn_in).toBe(true);
    expect(p.export_config.format).toBe("srt"); // unchanged
  });

  it("changes caption size", () => {
    const p = patchConfig(SAMPLE_PROJECT, { caption_size_px: 28 });
    expect(p.export_config.caption_size_px).toBe(28);
  });

  it("changes caption colour", () => {
    const p = patchConfig(SAMPLE_PROJECT, { caption_color: "yellow" });
    expect(p.export_config.caption_color).toBe("yellow");
  });

  it("changes caption background", () => {
    const p = patchConfig(SAMPLE_PROJECT, { caption_background: "none" });
    expect(p.export_config.caption_background).toBe("none");
  });

  it("changes max chars per line", () => {
    const p = patchConfig(SAMPLE_PROJECT, { max_chars_per_line: 52 });
    expect(p.export_config.max_chars_per_line).toBe(52);
  });

  it("does not mutate the input project", () => {
    const before = { ...SAMPLE_PROJECT.export_config };
    patchConfig(SAMPLE_PROJECT, { format: "ass", burn_in: true });
    expect(SAMPLE_PROJECT.export_config).toEqual(before);
  });
});

describe("ProjectMeta patch helper", () => {
  it("sets title without touching other fields", () => {
    const p = patchMeta(SAMPLE_PROJECT, { title: "Grace Sermon" });
    expect(p.project_meta.title).toBe("Grace Sermon");
    expect(p.project_meta.language).toBe("auto");
    expect(p.project_meta.description).toBe("");
  });

  it("sets description", () => {
    const p = patchMeta(SAMPLE_PROJECT, {
      description: "A sermon on soteriology",
    });
    expect(p.project_meta.description).toBe("A sermon on soteriology");
  });

  it("sets proper_nouns list", () => {
    const p = patchMeta(SAMPLE_PROJECT, {
      proper_nouns: "kerygma, Lars, soteriology",
    });
    expect(p.project_meta.proper_nouns).toBe("kerygma, Lars, soteriology");
  });

  it("sets language override", () => {
    const p = patchMeta(SAMPLE_PROJECT, { language: "no" });
    expect(p.project_meta.language).toBe("no");
  });

  it("does not mutate the input project", () => {
    const before = { ...SAMPLE_PROJECT.project_meta };
    patchMeta(SAMPLE_PROJECT, { title: "mutate test", language: "de" });
    expect(SAMPLE_PROJECT.project_meta).toEqual(before);
  });
});

describe("ExportConfig valid choices", () => {
  const VALID_FORMATS = ["srt", "vtt", "ass"];
  const VALID_SIZES = [16, 20, 24, 28];
  const VALID_COLORS = ["white", "yellow", "green"];
  const VALID_BACKGROUNDS = ["black", "semitransparent", "none"];
  const VALID_CHARS = [32, 42, 52];

  it("default format is in valid list", () => {
    expect(VALID_FORMATS).toContain(DEFAULT_EXPORT_CONFIG.format);
  });

  it("default size is in valid list", () => {
    expect(VALID_SIZES).toContain(DEFAULT_EXPORT_CONFIG.caption_size_px);
  });

  it("default colour is in valid list", () => {
    expect(VALID_COLORS).toContain(DEFAULT_EXPORT_CONFIG.caption_color);
  });

  it("default background is in valid list", () => {
    expect(VALID_BACKGROUNDS).toContain(
      DEFAULT_EXPORT_CONFIG.caption_background,
    );
  });

  it("default max chars is in valid list", () => {
    expect(VALID_CHARS).toContain(DEFAULT_EXPORT_CONFIG.max_chars_per_line);
  });
});
