// Zustand store for the canonical /import flow per
// `Promptibrary Design System/screens/03-import.html`.
//
// State machine collapses to: empty → detected → preview_ready →
// extracting → candidates_ready → saving → saved (+ fetch_failed /
// extraction_failed branches). No per-candidate editor — the spec
// mockup goes straight from candidates list (multi-select with
// checkboxes) to "save N prompts". Users edit after save via the
// canonical prompt-route editor.

import { create } from "zustand";

import type {
  CandidatePrompt,
  ExtractionFailure,
  ExtractionMode,
  FetchedSourceContent,
  SourceDetection,
} from "@/shared/api/ipc";

export type ImportPhase =
  | "empty"
  | "detected"
  | "preview_ready"
  | "extracting"
  | "candidates_ready"
  | "saving"
  | "saved"
  | "fetch_failed"
  | "extraction_failed";

export interface SavedRecord {
  id: string;
  slug: string;
  title: string;
}

interface ImportState {
  url: string;
  detection: SourceDetection | null;
  preview: FetchedSourceContent | null;
  candidates: CandidatePrompt[];
  selectedIndices: Set<number>;
  savedRecords: SavedRecord[];
  failure: ExtractionFailure | null;
  extractionMode: ExtractionMode;
  /** Free-form status line under the stage bar — drives `.extract-line`. */
  statusLine: string | null;
  phase: ImportPhase;

  setUrl: (url: string) => void;
  setDetection: (detection: SourceDetection | null) => void;
  setPreview: (content: FetchedSourceContent) => void;
  setExtracting: () => void;
  setCandidates: (candidates: CandidatePrompt[]) => void;
  toggleCandidate: (index: number) => void;
  setAllCandidatesSelected: (selected: boolean) => void;
  setExtractionMode: (mode: ExtractionMode) => void;
  setStatusLine: (line: string | null) => void;
  setSaving: () => void;
  setSaved: (saved: SavedRecord[]) => void;
  setFetchFailed: (failure: ExtractionFailure) => void;
  setExtractionFailed: (failure: ExtractionFailure) => void;
  reset: () => void;
}

const initial = {
  url: "",
  detection: null,
  preview: null,
  candidates: [] as CandidatePrompt[],
  selectedIndices: new Set<number>(),
  savedRecords: [] as SavedRecord[],
  failure: null,
  extractionMode: "standard" as ExtractionMode,
  statusLine: null,
  phase: "empty" as ImportPhase,
};

export const useImportStore = create<ImportState>((set, get) => ({
  ...initial,

  setUrl: (url) => set({ url, phase: "empty" }),

  setDetection: (detection) =>
    set({
      detection,
      phase: detection && detection.kind !== "unsupported" ? "detected" : "empty",
      failure: null,
    }),

  setPreview: (preview) =>
    set({ preview, phase: "preview_ready", failure: null, statusLine: null }),

  setExtracting: () =>
    set({
      phase: "extracting",
      failure: null,
      statusLine: "calling anthropic · streaming candidates",
    }),

  setCandidates: (candidates) =>
    // Default to every candidate selected — mirrors the mockup
    // (3/5 selected). The user un-checks the ones they don't want.
    set({
      candidates,
      selectedIndices: new Set(candidates.map((_, i) => i)),
      phase: "candidates_ready",
      failure: null,
      statusLine: null,
    }),

  toggleCandidate: (index) => {
    const next = new Set(get().selectedIndices);
    if (next.has(index)) {
      next.delete(index);
    } else {
      next.add(index);
    }
    set({ selectedIndices: next });
  },

  setAllCandidatesSelected: (selected) => {
    if (selected) {
      set({ selectedIndices: new Set(get().candidates.map((_, i) => i)) });
    } else {
      set({ selectedIndices: new Set() });
    }
  },

  setExtractionMode: (mode) => set({ extractionMode: mode }),

  setStatusLine: (statusLine) => set({ statusLine }),

  setSaving: () => set({ phase: "saving" }),

  setSaved: (savedRecords) => set({ phase: "saved", savedRecords }),

  setFetchFailed: (failure) => set({ phase: "fetch_failed", failure }),
  setExtractionFailed: (failure) => set({ phase: "extraction_failed", failure }),

  reset: () => set({ ...initial, selectedIndices: new Set() }),
}));
