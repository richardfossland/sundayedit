import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/react";

import { CleanupPanel } from "./CleanupPanel";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { useLocale } from "@/lib/i18n";
import type { Project, SilenceGap } from "@/lib/bindings";

// Mock the lowest layer (Tauri invoke) so the real typed `ipc` wrappers run —
// this also exercises the command names + argument shapes the panel relies on.
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

const GAPS: SilenceGap[] = [
  { start_ms: 1000, end_ms: 2500, duration_ms: 1500 },
  { start_ms: 4000, end_ms: 5200, duration_ms: 1200 },
];

beforeEach(() => {
  invoke.mockReset();
  // Assert against the English catalog regardless of the persisted default.
  useLocale.setState({ lang: "en" });
});

afterEach(() => {
  cleanup();
});

function renderPanel(onChange = vi.fn()) {
  render(<CleanupPanel project={SAMPLE_PROJECT} onProjectChange={onChange} />);
  return onChange;
}

describe("CleanupPanel — silence removal", () => {
  it("detects silences with the threshold and lists the gaps", async () => {
    invoke.mockResolvedValueOnce(GAPS); // detect_silences
    renderPanel();

    fireEvent.click(screen.getByText("Find silences"));
    // The detect call must hit the right command with the slider's default ms.
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("detect_silences", {
        project: SAMPLE_PROJECT,
        minGapMs: 1000,
      }),
    );

    // Both gaps render as ms-labelled rows (findByText throws if absent).
    expect(await screen.findByText("1500 ms gap")).toBeTruthy();
    expect(screen.getByText("1200 ms gap")).toBeTruthy();
  });

  it("ripple-cuts the approved gaps and feeds the new project back up", async () => {
    const nextProject: Project = { ...SAMPLE_PROJECT, updated_at: 99 };
    invoke
      .mockResolvedValueOnce(GAPS) // detect_silences
      .mockResolvedValueOnce(nextProject); // apply_ripple_cuts
    const onChange = renderPanel();

    fireEvent.click(screen.getByText("Find silences"));
    await screen.findByText("1500 ms gap");

    // Default = all approved → cuts derived from both gaps, in [start,end] form.
    fireEvent.click(screen.getByText("Remove 2 selected"));
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("apply_ripple_cuts", {
        project: SAMPLE_PROJECT,
        cuts: [
          [1000, 2500],
          [4000, 5200],
        ],
      }),
    );
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledWith(nextProject));
  });

  it("deselecting a gap excludes it from the cut set", async () => {
    invoke
      .mockResolvedValueOnce(GAPS)
      .mockResolvedValueOnce({ ...SAMPLE_PROJECT });
    renderPanel();

    fireEvent.click(screen.getByText("Find silences"));
    await screen.findByText("1500 ms gap");

    // Uncheck the first gap's checkbox.
    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);

    fireEvent.click(screen.getByText("Remove 1 selected"));
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("apply_ripple_cuts", {
        project: SAMPLE_PROJECT,
        cuts: [[4000, 5200]],
      }),
    );
  });

  it("shows an empty-state when no silences are found", async () => {
    invoke.mockResolvedValueOnce([]);
    renderPanel();

    fireEvent.click(screen.getByText("Find silences"));
    expect(await screen.findByText("No silences found 🎉")).toBeTruthy();
  });
});
