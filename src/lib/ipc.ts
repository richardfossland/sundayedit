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
  AppError,
  AsrOptions,
  BurnInOptions,
  Caption,
  ClaudeModel,
  ExportPreset,
  ExportWarning,
  FillerHit,
  FindMatch,
  FindOptions,
  GlossaryApplyResult,
  PolishEstimate,
  PolishResult,
  Project,
  ReplaceResult,
  SilenceGap,
  Strictness,
  Suggestion,
  StylePreset,
  TranslationLanguage,
  TranslationResult,
  VideoMetadata,
  WaveformData,
  WhisperModel,
  WhisperModelInfo,
} from "./bindings";

export class IPCError extends Error {
  readonly code: AppError["code"];
  constructor(err: AppError) {
    super(err.message);
    this.code = err.code;
    this.name = "IPCError";
  }
}

async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
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
  editWord: (
    project: Project,
    captionId: string,
    wordIndex: number,
    newText: string,
  ) =>
    call<Project>("op_edit_word", { project, captionId, wordIndex, newText }),
  lockWord: (
    project: Project,
    captionId: string,
    wordIndex: number,
    locked: boolean,
  ) => call<Project>("op_lock_word", { project, captionId, wordIndex, locked }),
  acceptAlternate: (
    project: Project,
    captionId: string,
    wordIndex: number,
    alternateIndex: number,
  ) =>
    call<Project>("op_accept_alternate", {
      project,
      captionId,
      wordIndex,
      alternateIndex,
    }),
  retimeWord: (
    project: Project,
    captionId: string,
    wordIndex: number,
    newStartMs: number,
    newEndMs: number,
  ) =>
    call<Project>("op_retime_word", {
      project,
      captionId,
      wordIndex,
      newStartMs,
      newEndMs,
    }),
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
  ass: (project: Project) => call<string>("export_ass", { project }),
  txt: (project: Project, includeSpeakers = false, stripEmpty = true) =>
    call<string>("export_txt", { project, includeSpeakers, stripEmpty }),
};

// ── Project lifecycle + video import (Phase 1) ───────────────────────────────
export const project = {
  probe: (path: string) => call<VideoMetadata>("video_probe", { path }),
  createFromVideo: (path: string) =>
    call<Project>("project_create_from_video", { path }),
  save: (proj: Project, path: string) =>
    call<void>("project_save", { project: proj, path }),
  open: (path: string) => call<Project>("project_open", { path }),
  waveform: (videoPath: string, cacheDir: string) =>
    call<WaveformData>("waveform_compute", { videoPath, cacheDir }),
  relink: (
    targetHash: string,
    searchDirs: string[],
    originalFilename?: string,
  ) =>
    call<string | null>("project_relink", {
      targetHash,
      searchDirs,
      originalFilename,
    }),
  acceptedExtensions: () => call<string[]>("accepted_media_extensions"),
};

// ── ASR / transcription (Phase 2) ────────────────────────────────────────────
export const asr = {
  listModels: () => call<WhisperModelInfo[]>("asr_list_models"),
  downloadedModels: (modelsDir: string) =>
    call<WhisperModel[]>("asr_downloaded_models", { modelsDir }),
  /** Listen for "transcribe-progress" events on the window while this runs. */
  transcribeLocal: (
    audioPath: string,
    modelsDir: string,
    model: WhisperModel,
    options: AsrOptions,
  ) =>
    call<Caption[]>("asr_transcribe_local", {
      audioPath,
      modelsDir,
      model,
      options,
    }),
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

// ── Find/replace + filler cleanup (Phase 7) ──────────────────────────────────
export const cleanup = {
  find: (project: Project, options: FindOptions) =>
    call<FindMatch[]>("find_in_project", { project, options }),
  replace: (project: Project, options: FindOptions, replacement: string) =>
    call<ReplaceResult>("replace_in_project", {
      project,
      options,
      replacement,
    }),
  bulkDelete: (project: Project, captionIds: string[]) =>
    call<Project>("bulk_delete_captions", { project, captionIds }),
  bulkSetSpeaker: (
    project: Project,
    captionIds: string[],
    speakerId: string | null,
  ) => call<Project>("bulk_set_speaker", { project, captionIds, speakerId }),
  detectFillers: (project: Project, language: string) =>
    call<FillerHit[]>("detect_fillers", { project, language }),
  detectSilences: (project: Project, minGapMs: number) =>
    call<SilenceGap[]>("detect_silences", { project, minGapMs }),
  applyRippleCuts: (project: Project, cuts: Array<[number, number]>) =>
    call<Project>("apply_ripple_cuts", { project, cuts }),
};

// ── AI punctuation polish (Phase 4.1) ────────────────────────────────────────
export const polish = {
  /** Pure cost/scope preview — no network, safe to call freely. */
  estimate: (project: Project, model: ClaudeModel) =>
    call<PolishEstimate>("polish_estimate", { project, model }),
  /** Run the polish. `apiKey` falls back to ANTHROPIC_API_KEY on the backend. */
  run: (project: Project, model: ClaudeModel, apiKey?: string) =>
    call<PolishResult>("polish_captions", {
      project,
      model,
      apiKey: apiKey ?? null,
    }),
};

// ── AI smart suggestions (Phase 4.3) ─────────────────────────────────────────
export const suggest = {
  /** Pure cost/scope preview — no network. */
  estimate: (project: Project, model: ClaudeModel, strictness: Strictness) =>
    call<PolishEstimate>("suggest_estimate", { project, model, strictness }),
  /** Run a Smart Suggest pass → review queue. Applies nothing. */
  run: (
    project: Project,
    model: ClaudeModel,
    strictness: Strictness,
    apiKey?: string,
  ) =>
    call<Suggestion[]>("suggest_captions", {
      project,
      model,
      strictness,
      apiKey: apiKey ?? null,
    }),
  /** Apply one accepted suggestion → updated project. */
  apply: (project: Project, suggestion: Suggestion) =>
    call<Project>("apply_suggestion", { project, suggestion }),
};

// ── AI translation (Phase 7.1) ───────────────────────────────────────────────
export const translate = {
  languages: () => call<TranslationLanguage[]>("translate_supported_languages"),
  /** Pure cost/scope preview — no network. */
  estimate: (project: Project, targetLanguage: string, model: ClaudeModel) =>
    call<PolishEstimate>("translate_estimate", {
      project,
      targetLanguage,
      model,
    }),
  /** Translate the track → result (captions + warnings). Does not mutate. */
  run: (
    project: Project,
    targetLanguage: string,
    model: ClaudeModel,
    apiKey?: string,
  ) =>
    call<TranslationResult>("translate_captions", {
      project,
      targetLanguage,
      model,
      apiKey: apiKey ?? null,
    }),
};

// ── Speaker diarization (Phase 4.2) ──────────────────────────────────────────
export const diarize = {
  /** Detect speakers from extracted audio + attribute captions. Best-effort. */
  run: (project: Project, audioPath: string) =>
    call<Project>("diarize_run", { project, audioPath }),
  mergeSpeakers: (project: Project, keepId: string, removeId: string) =>
    call<Project>("speaker_merge", { project, keepId, removeId }),
  renameSpeaker: (project: Project, speakerId: string, name: string) =>
    call<Project>("speaker_rename", { project, speakerId, name }),
  setSpeakerColor: (project: Project, speakerId: string, colorHex: string) =>
    call<Project>("speaker_set_color", { project, speakerId, colorHex }),
};

export const ipc = {
  ops,
  exporters,
  project,
  asr,
  style,
  render,
  cleanup,
  polish,
  suggest,
  translate,
  diarize,
};
