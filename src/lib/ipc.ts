/**
 * Typed wrappers around Tauri's invoke().
 *
 * For v1, the project state lives in the renderer; operations are pure
 * round-trips: send the project + the operation params, get the new
 * project back. This keeps the Rust layer stateless and makes undo a
 * matter of keeping previous states client-side.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AppError, AsrOptions, BurnInOptions, Caption, ExportPreset, ExportWarning,
  GlossaryApplyResult, Project, StylePreset, VideoMetadata, WaveformData,
  WhisperModel, WhisperModelInfo,
} from "./bindings";

export class IPCError extends Error {
  readonly code: AppError["code"];
  constructor(err: AppError) {
    super(err.message);
    this.code = err.code;
    this.name = "IPCError";
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (raw) {
    if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
      throw new IPCError(raw as AppError);
    }
    throw raw instanceof Error ? raw : new Error(String(raw));
  }
}

// ── Caption operations ────────────────────────────────────────────────────────
export const ops = {
  splitCaption: (project: Project, captionId: string, atWordIndex: number) =>
    call<Project>("op_split_caption", { project, captionId, atWordIndex }),
  mergeCaptions: (project: Project, captionIds: string[]) =>
    call<Project>("op_merge_captions", { project, captionIds }),
  shiftAll: (project: Project, offsetMs: number) =>
    call<Project>("op_shift_all_captions", { project, offsetMs }),
  editWord: (project: Project, captionId: string, wordIndex: number, newText: string) =>
    call<Project>("op_edit_word", { project, captionId, wordIndex, newText }),
  lockWord: (project: Project, captionId: string, wordIndex: number, locked: boolean) =>
    call<Project>("op_lock_word", { project, captionId, wordIndex, locked }),
  acceptAlternate: (project: Project, captionId: string, wordIndex: number, alternateIndex: number) =>
    call<Project>("op_accept_alternate", { project, captionId, wordIndex, alternateIndex }),
  retimeWord: (project: Project, captionId: string, wordIndex: number, newStartMs: number, newEndMs: number) =>
    call<Project>("op_retime_word", { project, captionId, wordIndex, newStartMs, newEndMs }),
  /** Killer feature #2 post-pass: apply glossary aliases → canonical terms. */
  applyGlossary: (project: Project) =>
    call<GlossaryApplyResult>("op_apply_glossary", { project }),
};

// ── Export ──────────────────────────────────────────────────────────────────
export const exporters = {
  srt: (project: Project, includeSpeakers = false, stripEmpty = true) =>
    call<string>("export_srt", { project, includeSpeakers, stripEmpty }),
  vtt: (project: Project, includeSpeakers = false, stripEmpty = true) =>
    call<string>("export_vtt", { project, includeSpeakers, stripEmpty }),
  ass: (project: Project) =>
    call<string>("export_ass", { project }),
  txt: (project: Project, includeSpeakers = false, stripEmpty = true) =>
    call<string>("export_txt", { project, includeSpeakers, stripEmpty }),
};

// ── Project lifecycle + video import (Phase 1) ───────────────────────────────
export const project = {
  probe: (path: string) =>
    call<VideoMetadata>("video_probe", { path }),
  createFromVideo: (path: string) =>
    call<Project>("project_create_from_video", { path }),
  save: (proj: Project, path: string) =>
    call<void>("project_save", { project: proj, path }),
  open: (path: string) =>
    call<Project>("project_open", { path }),
  waveform: (videoPath: string, cacheDir: string) =>
    call<WaveformData>("waveform_compute", { videoPath, cacheDir }),
  relink: (targetHash: string, searchDirs: string[], originalFilename?: string) =>
    call<string | null>("project_relink", { targetHash, searchDirs, originalFilename }),
  acceptedExtensions: () =>
    call<string[]>("accepted_media_extensions"),
};

// ── ASR / transcription (Phase 2) ────────────────────────────────────────────
export const asr = {
  listModels: () =>
    call<WhisperModelInfo[]>("asr_list_models"),
  downloadedModels: (modelsDir: string) =>
    call<WhisperModel[]>("asr_downloaded_models", { modelsDir }),
  /** Listen for "transcribe-progress" events on the window while this runs. */
  transcribeLocal: (audioPath: string, modelsDir: string, model: WhisperModel, options: AsrOptions) =>
    call<Caption[]>("asr_transcribe_local", { audioPath, modelsDir, model, options }),
};

// ── Styling (Phase 5) ─────────────────────────────────────────────────────────
export const style = {
  listPresets: () => call<StylePreset[]>("style_list_presets"),
};

// ── Burn-in + platform export (Phase 6.2 / 6.3) ──────────────────────────────
export const render = {
  listExportPresets: () => call<ExportPreset[]>("export_list_presets"),
  validate: (project: Project, preset: ExportPreset) =>
    call<ExportWarning[]>("export_validate", { project, preset }),
  burnIn: (project: Project, output: string, options: BurnInOptions) =>
    call<void>("burnin_render", { project, output, options }),
  burnInPreset: (project: Project, output: string, preset: ExportPreset) =>
    call<void>("burnin_render_preset", { project, output, preset }),
};

export const ipc = { ops, exporters, project, asr, style, render };
