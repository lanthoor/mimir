// Typed wrappers around `tauri.invoke`. Each function mirrors one
// `#[tauri::command]` defined in `crates/app/src/command.rs`.

import { invoke } from "@tauri-apps/api/core";
import type {
  AlbumRow,
  EditableTrackFields,
  FolderRow,
  FolderView,
  GenreRow,
  LibraryStatus,
  LyricsPayload,
  PlayerSnapshot,
  QueueItem,
  TrackPatch,
  TrackRow,
  YearRow,
} from "./types";

// ---- library -----------------------------------------------------------

export const libraryOpen = (path: string) =>
  invoke<void>("library_open", { path });

export const libraryStatus = () =>
  invoke<LibraryStatus>("library_status");

export const libraryListFolders = () =>
  invoke<FolderRow[]>("library_list_folders");

export const libraryFolderTree = () =>
  invoke<FolderView>("library_folder_tree");

export const libraryAddFolder = (path: string) =>
  invoke<number>("library_add_folder", { path });

export const libraryAddFolders = (paths: string[]) =>
  invoke<number[]>("library_add_folders", { paths });

export const librarySearch = (query: string, limit = 100) =>
  invoke<TrackRow[]>("library_search", { query, limit });

export const libraryListAlbums = (limit = 200, offset = 0) =>
  invoke<AlbumRow[]>("library_list_albums", { limit, offset });

export const libraryListGenres = () => invoke<GenreRow[]>("library_list_genres");

export const libraryListYears = () => invoke<YearRow[]>("library_list_years");

export const libraryListTracks = (limit = 100, offset = 0) =>
  invoke<TrackRow[]>("library_list_tracks", { limit, offset });

export const libraryQueryTracks = (params: {
  genre?: string | null;
  year?: number | null;
  artistId?: number | null;
  albumId?: number | null;
  limit?: number;
  offset?: number;
}) =>
  invoke<TrackRow[]>("library_query_tracks", {
    genre: params.genre,
    year: params.year,
    artistId: params.artistId,
    albumId: params.albumId,
    limit: params.limit ?? 100,
    offset: params.offset ?? 0,
  });

export const libraryGetEditableTrack = (trackId: number) =>
  invoke<EditableTrackFields>("library_get_editable_track", { trackId });

export const libraryUpdateTrack = (trackId: number, patch: TrackPatch) =>
  invoke<void>("library_update_track", { trackId, patch });

export const libraryClearTrackField = (trackId: number, field: string) =>
  invoke<void>("library_clear_track_field", { trackId, field });

export const libraryTrackLyrics = (trackId: number) =>
  invoke<LyricsPayload | null>("library_track_lyrics", { trackId });

export const libraryRemoveFolder = (folderId: number) =>
  invoke<void>("library_remove_folder", { folderId });

export const libraryRevealInFileManager = (path: string) =>
  invoke<void>("library_reveal_in_file_manager", { path });

// ---- audio ------------------------------------------------------------

export const audioPlay = (trackId: number) =>
  invoke<void>("audio_play", { trackId });

export const audioPause = () => invoke<void>("audio_pause");

export const audioResume = () => invoke<void>("audio_resume");

export const audioStop = () => invoke<void>("audio_stop");

export const audioNext = () => invoke<void>("audio_next");

export const audioPrevious = () => invoke<void>("audio_previous");

export const audioQueueEnqueueMany = (trackIds: number[]) =>
  invoke<number[]>("audio_queue_enqueue_many", { trackIds });

export const audioPlayAndEnqueueMany = (trackIds: number[]) =>
  invoke<number[]>("audio_play_and_enqueue_many", { trackIds });

export const audioQueueRemoveAt = (index: number) =>
  invoke<void>("audio_queue_remove_at", { index });

export const audioQueueMove = (from: number, to: number) =>
  invoke<void>("audio_queue_move", { from, to });

export const audioQueuePlayAt = (index: number) =>
  invoke<void>("audio_queue_play_at", { index });

export const audioQueueClear = () => invoke<void>("audio_queue_clear");

export const audioQueueGet = () => invoke<QueueItem[]>("audio_queue_get");

export const audioPlayerSnapshot = () =>
  invoke<PlayerSnapshot | null>("audio_player_snapshot");
