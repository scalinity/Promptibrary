// Source discriminated union per spec §4 "Source".

import type { IsoDateTime } from "./ids";
import type { SourceKind } from "./enums";

export interface SourceBase<TKind extends SourceKind> {
  kind: TKind;
  originUrl: string | null;
  title: string | null;
  author: string | null;
  fetchedAt: IsoDateTime | null;
  contentHash: string | null;
}

// Manual sources have no fetch origin — the `originUrl` field is dropped
// entirely so Rust and TS agree at compile time. Rust's ManualSource also
// omits the field (see src-tauri/src/domain/source.rs).
export type ManualSource = Omit<SourceBase<"manual">, "originUrl">;

export interface YouTubeSource extends SourceBase<"youtube"> {
  videoId: string;
  channelName: string | null;
  transcriptLanguage: string | null;
  durationSeconds: number | null;
}

export interface XTwitterSource extends SourceBase<"x_twitter"> {
  postId: string;
  username: string | null;
  threadPostIds: string[];
}

export interface ArticleSource extends SourceBase<"article"> {
  siteName: string | null;
  byline: string | null;
  publishedAt: IsoDateTime | null;
}

export type Source = ManualSource | YouTubeSource | XTwitterSource | ArticleSource;
