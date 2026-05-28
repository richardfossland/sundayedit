/**
 * Typed wrappers around Tauri's invoke().
 *
 * For v1, the project state lives in the renderer; operations are pure
 * round-trips: send the project + the operation params, get the new
 * project back. This keeps the Rust layer stateless and makes undo a
 * matter of keeping previous states client-side.
 */

import { invoke } from "@tauri-apps/api/core";
import type { AppError, Project } from "./bindings";

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

export const ipc = { ops, exporters };
