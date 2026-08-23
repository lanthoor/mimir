// Global UI state. Replaces the old vanilla `state` + `vstate()` +
// manual `render()` dance. Reads trigger re-renders in the components
// that call `useStore`.

import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type {
  AlbumRow,
  FolderView,
  GenreRow,
  LibraryStatus,
  PlayerSnapshot,
  TrackRow,
  YearRow,
} from "./types";

export type ViewKey =
  | "tracks"
  | "albums"
  | "artists"
  | "genres"
  | "years"
  | "folders"
  | "queue";

export type TracksMode = "icons" | "list";
export type AlbumsMode = "icons" | "list";
export type FoldersMode = "icons" | "tree";

export type TracksFilter = {
  genre: string | null;
  year: number | null;
  artistId: number | null;
  albumId: number | null;
};

type Store = {
  // View routing
  view: ViewKey;
  setView: (v: ViewKey) => void;

  // Per-view state
  tracks: {
    mode: TracksMode;
    query: string;
    filter: TracksFilter;
  };
  setTracksMode: (m: TracksMode) => void;
  setTracksQuery: (q: string) => void;
  setTracksFilter: (f: TracksFilter) => void;

  albums: {
    mode: AlbumsMode;
    selectedAlbumId: number | null;
  };
  setAlbumsMode: (m: AlbumsMode) => void;
  selectAlbum: (id: number | null) => void;

  folders: {
    mode: FoldersMode;
    // Path of the directory we're inside; null = roots.
    cwd: string | null;
  };
  setFoldersMode: (m: FoldersMode) => void;
  setCwd: (path: string | null) => void;

  // Library status
  library: LibraryStatus;
  setLibrary: (s: LibraryStatus) => void;

  // Loading
  loading: boolean;
  setLoading: (v: boolean) => void;

  // Scan progress
  scanning: number;
  bumpScanning: (delta: number) => void;

  // Data per view
  tracksList: TrackRow[];
  setTracksList: (rows: TrackRow[]) => void;
  albumsList: AlbumRow[];
  setAlbumsList: (rows: AlbumRow[]) => void;
  genresList: GenreRow[];
  setGenresList: (rows: GenreRow[]) => void;
  yearsList: YearRow[];
  setYearsList: (rows: YearRow[]) => void;
  folderTree: FolderView;
  setFolderTree: (tree: FolderView) => void;

  // Audio
  nowPlayingTrackId: number | null;
  setNowPlayingTrackId: (id: number | null) => void;
  nowPlayingTitle: string;
  nowPlayingArtist: string;
  setNowPlaying: (title: string, artist: string) => void;
  playerSnapshot: PlayerSnapshot | null;
  setPlayerSnapshot: (s: PlayerSnapshot | null) => void;
  lyricsTrackId: number | null;
  setLyricsTrackId: (id: number | null) => void;
};

const initialTracksFilter: TracksFilter = {
  genre: null,
  year: null,
  artistId: null,
  albumId: null,
};

export const useStore = create<Store>()(
  subscribeWithSelector((set) => ({
    view: "tracks",
    setView: (v) => set({ view: v }),

    tracks: { mode: "icons", query: "", filter: { ...initialTracksFilter } },
    setTracksMode: (mode) =>
      set((s) => ({ tracks: { ...s.tracks, mode } })),
    setTracksQuery: (query) =>
      set((s) => ({ tracks: { ...s.tracks, query } })),
    setTracksFilter: (filter) =>
      set((s) => ({ tracks: { ...s.tracks, filter } })),

    albums: { mode: "icons", selectedAlbumId: null },
    setAlbumsMode: (mode) =>
      set((s) => ({ albums: { ...s.albums, mode } })),
    selectAlbum: (id) =>
      set((s) => ({ albums: { ...s.albums, selectedAlbumId: id } })),

    folders: { mode: "icons", cwd: null },
    setFoldersMode: (mode) =>
      set((s) => ({ folders: { ...s.folders, mode } })),
    setCwd: (cwd) => set((s) => ({ folders: { ...s.folders, cwd } })),

    library: { path: null, last_error: null },
    setLibrary: (library) => set({ library }),

    loading: false,
    setLoading: (loading) => set({ loading }),

    scanning: 0,
    bumpScanning: (delta) => set((s) => ({ scanning: s.scanning + delta })),

    tracksList: [],
    setTracksList: (tracksList) => set({ tracksList }),
    albumsList: [],
    setAlbumsList: (albumsList) => set({ albumsList }),
    genresList: [],
    setGenresList: (genresList) => set({ genresList }),
    yearsList: [],
    setYearsList: (yearsList) => set({ yearsList }),
    folderTree: { flat: [], root_children: [] },
    setFolderTree: (folderTree) => set({ folderTree }),

    nowPlayingTrackId: null,
    setNowPlayingTrackId: (nowPlayingTrackId) => set({ nowPlayingTrackId }),
    nowPlayingTitle: "—",
    nowPlayingArtist: "",
    setNowPlaying: (nowPlayingTitle, nowPlayingArtist) =>
      set({ nowPlayingTitle, nowPlayingArtist }),
    playerSnapshot: null,
    setPlayerSnapshot: (playerSnapshot) => set({ playerSnapshot }),
    lyricsTrackId: null,
    setLyricsTrackId: (lyricsTrackId) => set({ lyricsTrackId }),
  })),
);
