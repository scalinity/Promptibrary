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

export interface ManualSource extends SourceBase<"manual"> {
  originUrl: null;
}

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
