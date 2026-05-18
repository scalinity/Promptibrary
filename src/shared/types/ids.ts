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

// Cast helpers for runtime values whose shape is checked elsewhere
// (e.g. ULIDs are validated by Rust before they cross IPC).
export const asPromptId = (v: string): PromptId => v as PromptId;
export const asRunId = (v: string): RunId => v as RunId;
export const asSourceId = (v: string): SourceId => v as SourceId;
export const asTagName = (v: string): TagName => v as TagName;
export const asRelativeVaultPath = (v: string): RelativeVaultPath =>
  v as RelativeVaultPath;
export const asAbsolutePath = (v: string): AbsolutePath => v as AbsolutePath;
export const asIsoDateTime = (v: string): IsoDateTime => v as IsoDateTime;
