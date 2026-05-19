// Zustand store for the /import state machine per spec §12.
//
// State flow:
//   Empty  ─url→ Detected ─fetch→ PreviewReady ─extract→ Extracting
//                                                            │
//                                                            ▼
//                                                     CandidatesReady
//                                                            │
//                                          ─pick(idx)→ EditingCandidate
//                                                            │
//                                          ─save────→ Saved
//
//   FetchFailed / ExtractionFailed are terminal-ish; user resets via the
//   panel and starts over by entering a new URL.

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
  | "editing_candidate"
  | "saved"
  | "fetch_failed"
  | "extraction_failed";

interface ImportState {
  url: string;
  detection: SourceDetection | null;
  preview: FetchedSourceContent | null;
  candidates: CandidatePrompt[];
  selectedCandidateIndex: number | null;
  /** Live-edited candidate (candidate-editor patches; save commits). */
  draft: CandidatePrompt | null;
  savedPromptId: string | null;
  failure: ExtractionFailure | null;
  extractionMode: ExtractionMode;
  phase: ImportPhase;

  setUrl: (url: string) => void;
  setDetection: (detection: SourceDetection | null) => void;
  setPreview: (content: FetchedSourceContent) => void;
  setExtracting: () => void;
  setCandidates: (candidates: CandidatePrompt[]) => void;
  selectCandidate: (index: number) => void;
  patchDraft: (patch: Partial<CandidatePrompt>) => void;
  setExtractionMode: (mode: ExtractionMode) => void;
  setFetchFailed: (failure: ExtractionFailure) => void;
  setExtractionFailed: (failure: ExtractionFailure) => void;
  setSaved: (promptId: string) => void;
  reset: () => void;
}

const initial = {
  url: "",
  detection: null,
  preview: null,
  candidates: [] as CandidatePrompt[],
  selectedCandidateIndex: null,
  draft: null,
  savedPromptId: null,
  failure: null,
  extractionMode: "standard" as ExtractionMode,
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
    set({ preview, phase: "preview_ready", failure: null }),

  setExtracting: () => set({ phase: "extracting", failure: null }),

  setCandidates: (candidates) =>
    set({
      candidates,
      phase: "candidates_ready",
      selectedCandidateIndex: null,
      draft: null,
      failure: null,
    }),

  selectCandidate: (index) => {
    const candidate = get().candidates[index];
    if (!candidate) return;
    set({
      selectedCandidateIndex: index,
      draft: { ...candidate },
      phase: "editing_candidate",
    });
  },

  patchDraft: (patch) => {
    const draft = get().draft;
    if (!draft) return;
    set({ draft: { ...draft, ...patch } });
  },

  setExtractionMode: (mode) => set({ extractionMode: mode }),

  setFetchFailed: (failure) => set({ phase: "fetch_failed", failure }),
  setExtractionFailed: (failure) => set({ phase: "extraction_failed", failure }),

  setSaved: (promptId) => set({ phase: "saved", savedPromptId: promptId }),

  reset: () => set({ ...initial }),
}));
