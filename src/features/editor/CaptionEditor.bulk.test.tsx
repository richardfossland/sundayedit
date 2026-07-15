import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/react";

import { CaptionEditor } from "./CaptionEditor";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { useProjectStore } from "@/lib/useProjectStore";
import { useLocale } from "@/lib/i18n";
import type { Project } from "@/lib/bindings";

// Mock the lowest layer (Tauri invoke) so the real typed `ipc` wrappers run —
// this pins the command names + argument shapes the bulk bar relies on.
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

beforeEach(() => {
  invoke.mockReset();
  useLocale.setState({ lang: "en" });
  // The editor now sources its project from the shared store; seed a clean one.
  useProjectStore.setState({
    project: SAMPLE_PROJECT,
    past: [],
    future: [],
    busy: false,
    inFlight: false,
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** Render the store-backed editor (project seeded in beforeEach). */
function renderEditor() {
  render(<CaptionEditor />);
}

/** The project the store currently holds (what an op commits to). */
function storeProject(): Project | null {
  return useProjectStore.getState().project;
}

/** The per-row select checkboxes, in render order (c1, c2, c3). */
function selectBoxes() {
  return screen
    .getAllByRole("checkbox")
    .filter((el) => el.getAttribute("aria-label") === "Select caption");
}

describe("CaptionEditor — bulk action bar", () => {
  it("shows the bar with a live count once captions are selected", () => {
    renderEditor();
    expect(screen.queryByText(/selected/)).toBeNull();

    fireEvent.click(selectBoxes()[0]);
    expect(screen.getByText("1 selected")).toBeTruthy();

    fireEvent.click(selectBoxes()[1]);
    expect(screen.getByText("2 selected")).toBeTruthy();
  });

  it("merges contiguous captions through op_merge_captions", async () => {
    const next: Project = { ...SAMPLE_PROJECT, updated_at: 1 };
    invoke.mockResolvedValueOnce(next); // op_merge_captions
    renderEditor();

    // c1 + c2 are adjacent → Merge enabled.
    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(selectBoxes()[1]);

    const merge = screen.getByRole("button", { name: /Merge/ });
    expect(merge.hasAttribute("disabled")).toBe(false);
    fireEvent.click(merge);

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("op_merge_captions", {
        project: SAMPLE_PROJECT,
        captionIds: ["c1", "c2"],
      }),
    );
    // The op commits to the shared store (undoable).
    await vi.waitFor(() => expect(storeProject()).toEqual(next));
  });

  it("disables Merge for a non-contiguous selection (no round-trip)", () => {
    renderEditor();
    // c1 + c3 → gap at c2 → not mergeable.
    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(selectBoxes()[2]);

    const merge = screen.getByRole("button", { name: /Merge/ });
    expect(merge.hasAttribute("disabled")).toBe(true);
    fireEvent.click(merge);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("bulk-deletes the selected captions through bulk_delete_captions", async () => {
    const next: Project = { ...SAMPLE_PROJECT, updated_at: 2 };
    invoke.mockResolvedValueOnce(next);
    renderEditor();

    fireEvent.click(selectBoxes()[1]);
    fireEvent.click(selectBoxes()[2]);
    fireEvent.click(screen.getByRole("button", { name: /Delete/ }));

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("bulk_delete_captions", {
        project: SAMPLE_PROJECT,
        captionIds: ["c2", "c3"],
      }),
    );
    await vi.waitFor(() => expect(storeProject()).toEqual(next));
  });

  it("sets a speaker on the selection through bulk_set_speaker", async () => {
    const next: Project = { ...SAMPLE_PROJECT, updated_at: 3 };
    invoke.mockResolvedValueOnce(next);
    renderEditor();

    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(screen.getByRole("button", { name: /Speaker/ }));
    // The project has one speaker, "Pastor Lars" (id s1).
    fireEvent.click(screen.getByText("Pastor Lars"));

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("bulk_set_speaker", {
        project: SAMPLE_PROJECT,
        captionIds: ["c1"],
        speakerId: "s1",
      }),
    );
  });

  it("clears the speaker (null) via the 'No speaker' choice", async () => {
    invoke.mockResolvedValueOnce({ ...SAMPLE_PROJECT });
    renderEditor();

    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(screen.getByRole("button", { name: /Speaker/ }));
    fireEvent.click(screen.getByText("No speaker"));

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("bulk_set_speaker", {
        project: SAMPLE_PROJECT,
        captionIds: ["c1"],
        speakerId: null,
      }),
    );
  });

  it("shift-click selects a contiguous range", () => {
    renderEditor();
    fireEvent.click(selectBoxes()[0]); // anchor on c1
    fireEvent.click(selectBoxes()[2], { shiftKey: true }); // range c1..c3
    expect(screen.getByText("3 selected")).toBeTruthy();
    // Whole run is contiguous → Merge enabled.
    expect(
      screen.getByRole("button", { name: /Merge/ }).hasAttribute("disabled"),
    ).toBe(false);
  });

  it("select-all then clear toggles the whole selection", () => {
    renderEditor();
    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(screen.getByText("Select all"));
    expect(screen.getByText("3 selected")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.queryByText(/selected/)).toBeNull();
  });

  it("shifts the whole project through op_shift_all_captions", async () => {
    const next: Project = { ...SAMPLE_PROJECT, updated_at: 4 };
    invoke.mockResolvedValueOnce(next);
    const promptSpy = vi.spyOn(window, "prompt").mockReturnValue("250");
    renderEditor();

    fireEvent.click(selectBoxes()[0]);
    fireEvent.click(
      screen.getByRole("button", { name: /Shift whole project/ }),
    );

    expect(promptSpy).toHaveBeenCalled();
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("op_shift_all_captions", {
        project: SAMPLE_PROJECT,
        offsetMs: 250,
      }),
    );
  });
});
