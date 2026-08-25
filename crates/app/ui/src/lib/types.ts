// TypeScript mirrors of the Rust IPC payloads defined in
// `crates/app/src/command.rs` and the `mimir-core::query` structs.
// Keep in sync with the serde-serialized types on the backend.

export type TrackPatch = {
  title?: string | null;
  genre?: string | null;
  year?: number | null;
  track_no?: number | null;
  disc_no?: number | null;
  clear: string[];
};

export type TrackRow = {
  id: number;
  path: string;
  title: string | null;
  track_no: number | null;
  disc_no: number | null;
  duration_ms: number | null;
  codec: string;
  genre: string | null;
  year: number | null;
  album_id: number | null;
  album_title: string | null;
  artist_id: number | null;
  artist_name: string | null;
};

export type AlbumRow = {
  id: number;
  title: string;
  year: number | null;
  artist_id: number | null;
  artist_name: string | null;
  track_count: number;
};

export type GenreRow = {
  name: string;
  track_count: number;
};

export type YearRow = {
  year: number;
  track_count: number;
};

export type ArtistRow = {
  id: number;
  name: string;
  sort_name: string | null;
  track_count: number;
};

export type FolderFile = {
  path: string;
  title: string | null;
  track_id: number | null;
};

export type FolderNode = {
  folder_id: number | null;
  name: string | null;
  path: string;
  files: FolderFile[];
  children: FolderNode[];
};

export type FolderView = {
  flat: FolderNode[];
  root_children: FolderNode[];
};

export type ScanSummary = {
  walked: number;
  sent: number;
  known: number;
  hashed_fail: number;
};

export type ScanDonePayload = {
  path: string;
  summary: ScanSummary;
};

export type ScanErrorPayload = {
  path: string;
  error: string;
};

export type LibraryStatus = {
  path: string | null;
  last_error: string | null;
};

export type EditableTrackFields = {
  title: string | null;
  genre: string | null;
  year: number | null;
  track_no: number | null;
  disc_no: number | null;
};

export type LyricsPayload = {
  text: string;
  language: string;
  source: string;
};

export type FolderRow = {
  id: number;
  path: string;
  file_count: number;
};

export type ListFolderRow = {
  id: number;
  path: string;
};

export type TrackSearchPage = {
  rows: TrackRow[];
  total: number;
};

export type PlayerSnapshot = {
  state: string;
  current: string | null;
  position_secs: number;
  total_secs: number;
};

export type QueueItem = {
  index: number;
  track_id: number;
  title: string;
  artist_name: string | null;
  is_current: boolean;
};
