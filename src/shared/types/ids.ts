// Branded identifier aliases per spec §4.
//
// The Brand pattern prevents accidental cross-typing — a `RunId` is structurally
// a `string`, but TypeScript's nominal-typing-by-property keeps it from being
// passed where a `PromptId` is expected.

export type Brand<T, B extends string> = T & { readonly __brand: B };

export type PromptId = Brand<string, "PromptId">; // ULID
export type RunId = Brand<string, "RunId">; // ULID
export type SourceId = Brand<string, "SourceId">; // ULID
export type TagName = Brand<string, "TagName">;
export type RelativeVaultPath = Brand<string, "RelativeVaultPath">;
export type AbsolutePath = Brand<string, "AbsolutePath">;
export type IsoDateTime = Brand<string, "IsoDateTime">;

// Cast helpers for runtime values whose shape is validated upstream.
//
// SCA-757 trust posture: these are unchecked `as` casts at the IPC
// boundary. ULID validity is enforced on the Rust side (the schema
// declares the column types and `crate::ids::PromptId(String)` only
// receives values produced by `crate::ids::new_ulid()` or read from
// the SQLite `prompts.id PRIMARY KEY` column). The TS side trusts
// that contract; we do not re-validate the regex `[0-9A-Z]{26}` here.
//
// Use these helpers only where the value crosses IPC from Rust, OR
// where you've already validated it through a typed surface (e.g.
// `Prompt.id` from `getPrompt(...)`). Building a `PromptId` from
// frontend-only state — a URL parameter, a clipboard paste, a free-
// text input — should NOT use `asPromptId` directly; route through
// the backend (which will reject malformed ids with PromptNotFound).
export const asPromptId = (v: string): PromptId => v as PromptId;
export const asRunId = (v: string): RunId => v as RunId;
export const asSourceId = (v: string): SourceId => v as SourceId;
export const asTagName = (v: string): TagName => v as TagName;
export const asRelativeVaultPath = (v: string): RelativeVaultPath =>
  v as RelativeVaultPath;
export const asAbsolutePath = (v: string): AbsolutePath => v as AbsolutePath;
export const asIsoDateTime = (v: string): IsoDateTime => v as IsoDateTime;
