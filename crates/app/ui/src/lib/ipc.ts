// Typed wrappers around `tauri.invoke`. Each function mirrors one
// `#[tauri::command]` defined in `crates/app/src/command.rs`.

import { invoke } from "@tauri-apps/api/core";
import type {
  AlbumRow,
  ArtistRow,
  EditableTrackFields,
  FolderFile,
  FolderRow,
  FolderView,
  GenreRow,
  LibraryStatus,
  ListFolderRow,
  LyricsPayload,
  PlayerSnapshot,
  QueueItem,
  TrackPatch,
  TrackRow,
  TrackSearchPage,
  YearRow,
} from "./types";

// ---- library -----------------------------------------------------------

export const libraryOpen = (path: string) =>
  invoke<void>("library_open", { path });

export const libraryStatus = () =>
  invoke<LibraryStatus>("library_status");

export const libraryListFolders = () =>
  invoke<FolderRow[]>("library_list_folders");

export const libraryListListedFolders = (limit = 50, offset = 0) =>
  invoke<ListFolderRow[]>("library_list_listed_folders", { limit, offset });

export const libraryCountListedFolders = () =>
  invoke<number>("library_count_listed_folders");

export const libraryListFolderFiles = (
  folderId: number,
  limit = 50,
  offset = 0,
) => invoke<FolderFile[]>("library_list_folder_files", { folderId, limit, offset });

export const libraryCountFolderFiles = (folderId: number) =>
  invoke<number>("library_count_folder_files", { folderId });

export const libraryFolderTree = () =>
  invoke<FolderView>("library_folder_tree");

export const libraryAddFolder = (path: string) =>
  invoke<number>("library_add_folder", { path });

export const libraryAddFolders = (paths: string[]) =>
  invoke<number[]>("library_add_folders", { paths });

export const librarySearch = (
  query: string,
  limit = 50,
  offset = 0,
) => invoke<TrackRow[]>("library_search", { query, limit, offset });

export const librarySearchTracksPage = (
  query: string,
  limit = 50,
  offset = 0,
) =>
  invoke<TrackSearchPage>("library_search_tracks_page", {
    query,
    limit,
    offset,
  });

export const libraryListAlbums = (limit = 50, offset = 0) =>
  invoke<AlbumRow[]>("library_list_albums", { limit, offset });

export const libraryCountAlbums = () => invoke<number>("library_count_albums");

export const libraryListGenres = (limit = 50, offset = 0) =>
  invoke<GenreRow[]>("library_list_genres", { limit, offset });

export const libraryCountGenres = () => invoke<number>("library_count_genres");

export const libraryListArtists = (limit = 50, offset = 0) =>
  invoke<ArtistRow[]>("library_list_artists", { limit, offset });

export const libraryCountArtists = () =>
  invoke<number>("library_count_artists");

export const libraryListYears = (limit = 50, offset = 0) =>
  invoke<YearRow[]>("library_list_years", { limit, offset });

export const libraryCountYears = () => invoke<number>("library_count_years");

export const libraryListTracks = (limit = 50, offset = 0) =>
  invoke<TrackRow[]>("library_list_tracks", { limit, offset });

export const libraryCountTracks = () => invoke<number>("library_count_tracks");

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
    limit: params.limit ?? 50,
    offset: params.offset ?? 0,
  });

export const libraryCountTracksFiltered = (params: {
  genre?: string | null;
  year?: number | null;
  artistId?: number | null;
  albumId?: number | null;
}) =>
  invoke<number>("library_count_tracks_filtered", {
    genre: params.genre,
    year: params.year,
    artistId: params.artistId,
    albumId: params.albumId,
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
