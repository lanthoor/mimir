// Global UI state. Replaces the old vanilla `state` + `vstate()` +
// manual `render()` dance. Reads trigger re-renders in the components
// that call `useStore`.

import { create } from "zustand";
import { persist, subscribeWithSelector } from "zustand/middleware";
import type {
  AlbumRow,
  ArtistRow,
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

/** Pagination slot for one view. Lives in the store so it survives
 *  navigation back from another view. Only `page` + `pageSize` are
 *  persisted to localStorage; `total` is recomputed on every mount. */
export type PageState = {
  page: number;
  pageSize: number;
  total: number;
};

export const DEFAULT_PAGE_SIZE = 50;

export const DEFAULT_PAGE = (): PageState => ({
  page: 1,
  pageSize: DEFAULT_PAGE_SIZE,
  total: 0,
});

/** Clamp `page` into [1, lastPage]. Returns 1 when total is 0. */
export function clampPageTo(page: number, total: number, pageSize: number) {
  if (total <= 0) return 1;
  const last = Math.max(1, Math.ceil(total / pageSize));
  if (page < 1) return 1;
  if (page > last) return last;
  return page;
}

type Store = {
  // View routing
  view: ViewKey;
  setView: (v: ViewKey) => void;

  // Per-view state
  tracks: {
    mode: TracksMode;
    query: string;
    filter: TracksFilter;
    page: PageState;
  };
  setTracksMode: (m: TracksMode) => void;
  setTracksQuery: (q: string) => void;
  setTracksFilter: (f: TracksFilter) => void;
  setTracksPage: (p: PageState["page"]) => void;
  setTracksTotal: (total: PageState["total"]) => void;
  setTracksPageSize: (size: PageState["pageSize"]) => void;

  albums: {
    mode: AlbumsMode;
    selectedAlbumId: number | null;
    page: PageState;
  };
  setAlbumsMode: (m: AlbumsMode) => void;
  selectAlbum: (id: number | null) => void;
  setAlbumsPage: (p: PageState["page"]) => void;
  setAlbumsTotal: (total: PageState["total"]) => void;
  setAlbumsPageSize: (size: PageState["pageSize"]) => void;

  artists: { page: PageState };
  setArtistsPage: (p: PageState["page"]) => void;
  setArtistsTotal: (total: PageState["total"]) => void;
  setArtistsPageSize: (size: PageState["pageSize"]) => void;

  genres: { page: PageState };
  setGenresPage: (p: PageState["page"]) => void;
  setGenresTotal: (total: PageState["total"]) => void;
  setGenresPageSize: (size: PageState["pageSize"]) => void;

  years: { page: PageState };
  setYearsPage: (p: PageState["page"]) => void;
  setYearsTotal: (total: PageState["total"]) => void;
  setYearsPageSize: (size: PageState["pageSize"]) => void;

  folders: {
    mode: FoldersMode;
    // Path of the directory we're inside; null = roots.
    cwd: string | null;
    // Pagination of the per-cwd file grid (icons mode only).
    page: PageState;
  };
  setFoldersMode: (m: FoldersMode) => void;
  setCwd: (path: string | null) => void;
  setFoldersPage: (p: PageState["page"]) => void;
  setFoldersTotal: (total: PageState["total"]) => void;
  setFoldersPageSize: (size: PageState["pageSize"]) => void;

  // Library status
  library: LibraryStatus;
  setLibrary: (s: LibraryStatus) => void;

  // Loading
  loading: boolean;
  setLoading: (v: boolean) => void;

  // Scan progress
  scanning: number;
  bumpScanning: (delta: number) => void;

  // Tick counter incremented on every successful scan; per-view pagination
  // hooks depend on it so they re-fetch after scans even when nothing
  // else changed.
  refreshTick: number;
  bumpRefreshTick: () => void;

  // Single-track metadata changes (in-place edit) don't bump the scan
  // tick but should refresh the affected page so the row reflects the
  // new title/genre/etc.
  bumpTracksRefresh: () => void;

  // Data per view
  tracksList: TrackRow[];
  setTracksList: (rows: TrackRow[]) => void;
  albumsList: AlbumRow[];
  setAlbumsList: (rows: AlbumRow[]) => void;
  genresList: GenreRow[];
  setGenresList: (rows: GenreRow[]) => void;
  yearsList: YearRow[];
  setYearsList: (rows: YearRow[]) => void;
  artistsList: ArtistRow[];
  setArtistsList: (rows: ArtistRow[]) => void;
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

// Pages slice shape used both for runtime typing and for `persist`'s
// `partialize` (only page + pageSize go into localStorage; total is
// recomputed on every mount).
const PAGES_KEYS = [
  "tracks",
  "albums",
  "artists",
  "genres",
  "years",
  "folders",
] as const;

function pageOnlySlice(s: Store) {
  const out: Record<string, unknown> = {};
  for (const k of PAGES_KEYS) {
    const v = s[k];
    if (v && typeof v === "object" && "page" in v) {
      const slot = v as { page: PageState };
      out[k] = {
        page: { page: slot.page.page, pageSize: slot.page.pageSize },
      };
    }
  }
  return out;
}

/// Merge persisted page slice over the live store. The default zustand
/// `persist` merge is a top-level shallow spread, which would replace
/// the entire `tracks` slot (and each other view slot) with the
/// persisted `{ page: {...} }` shape — wiping out `mode`, `query`,
/// `filter` and crashing the page-fetch hook. We deep-merge each named
/// view slot so persisted page state rides alongside the live state.
function mergePersisted(
  persisted: unknown,
  current: Store,
): Store {
  const out: Record<string, unknown> = {
    ...(current as unknown as Record<string, unknown>),
  };
  if (
    persisted &&
    typeof persisted === "object" &&
    !Array.isArray(persisted)
  ) {
    const p = persisted as Record<string, unknown>;
    for (const k of PAGES_KEYS) {
      const slot = p[k];
      if (
        slot &&
        typeof slot === "object" &&
        "page" in slot &&
        !Array.isArray(slot)
      ) {
        const persistedPage = (slot as { page: Partial<PageState> }).page;
        const liveSlot = (current as unknown as Record<string, unknown>)[k] as
          | { page: PageState }
          | undefined;
        if (liveSlot && typeof liveSlot === "object" && "page" in liveSlot) {
          const next = liveSlot.page;
          out[k] = {
            ...liveSlot,
            page: {
              ...next,
              page: persistedPage?.page ?? next.page,
              pageSize: persistedPage?.pageSize ?? next.pageSize,
            },
          };
        }
      }
    }
  }
  return out as unknown as Store;
}

export const useStore = create<Store>()(
  persist(
    subscribeWithSelector((set) => ({
      view: "tracks",
      setView: (view) => set({ view }),

      tracks: {
        mode: "icons",
        query: "",
        filter: { ...initialTracksFilter },
        page: DEFAULT_PAGE(),
      },
      setTracksMode: (mode) =>
        set((s) => ({ tracks: { ...s.tracks, mode } })),
      setTracksQuery: (query) =>
        set((s) => ({ tracks: { ...s.tracks, query } })),
      setTracksFilter: (filter) =>
        set((s) => ({ tracks: { ...s.tracks, filter } })),
      setTracksPage: (page) =>
        set((s) => ({ tracks: { ...s.tracks, page: { ...s.tracks.page, page } } })),
      setTracksTotal: (total) =>
        set((s) => ({
          tracks: { ...s.tracks, page: { ...s.tracks.page, total } },
        })),
      setTracksPageSize: (pageSize) =>
        set((s) => ({
          tracks: {
            ...s.tracks,
            page: {
              ...s.tracks.page,
              pageSize,
              page: clampPageTo(
                s.tracks.page.page,
                s.tracks.page.total,
                pageSize,
              ),
            },
          },
        })),

      albums: { mode: "icons", selectedAlbumId: null, page: DEFAULT_PAGE() },
      setAlbumsMode: (mode) =>
        set((s) => ({ albums: { ...s.albums, mode } })),
      selectAlbum: (id) =>
        set((s) => ({ albums: { ...s.albums, selectedAlbumId: id } })),
      setAlbumsPage: (page) =>
        set((s) => ({ albums: { ...s.albums, page: { ...s.albums.page, page } } })),
      setAlbumsTotal: (total) =>
        set((s) => ({
          albums: { ...s.albums, page: { ...s.albums.page, total } },
        })),
      setAlbumsPageSize: (pageSize) =>
        set((s) => ({
          albums: {
            ...s.albums,
            page: { ...s.albums.page, pageSize },
          },
        })),

      artists: { page: DEFAULT_PAGE() },
      setArtistsPage: (page) =>
        set((s) => ({ artists: { ...s.artists, page: { ...s.artists.page, page } } })),
      setArtistsTotal: (total) =>
        set((s) => ({
          artists: { ...s.artists, page: { ...s.artists.page, total } },
        })),
      setArtistsPageSize: (pageSize) =>
        set((s) => ({
          artists: { ...s.artists, page: { ...s.artists.page, pageSize } },
        })),

      genres: { page: DEFAULT_PAGE() },
      setGenresPage: (page) =>
        set((s) => ({ genres: { ...s.genres, page: { ...s.genres.page, page } } })),
      setGenresTotal: (total) =>
        set((s) => ({
          genres: { ...s.genres, page: { ...s.genres.page, total } },
        })),
      setGenresPageSize: (pageSize) =>
        set((s) => ({
          genres: { ...s.genres, page: { ...s.genres.page, pageSize } },
        })),

      years: { page: DEFAULT_PAGE() },
      setYearsPage: (page) =>
        set((s) => ({ years: { ...s.years, page: { ...s.years.page, page } } })),
      setYearsTotal: (total) =>
        set((s) => ({
          years: { ...s.years, page: { ...s.years.page, total } },
        })),
      setYearsPageSize: (pageSize) =>
        set((s) => ({
          years: { ...s.years, page: { ...s.years.page, pageSize } },
        })),

      folders: {
        mode: "icons",
        cwd: null,
        page: DEFAULT_PAGE(),
      },
      setFoldersMode: (mode) =>
        set((s) => ({ folders: { ...s.folders, mode } })),
      setCwd: (cwd) =>
        set((s) => ({
          folders: {
            ...s.folders,
            cwd,
            page: { ...s.folders.page, page: 1, total: 0 },
          },
        })),
      setFoldersPage: (page) =>
        set((s) => ({
          folders: { ...s.folders, page: { ...s.folders.page, page } },
        })),
      setFoldersTotal: (total) =>
        set((s) => ({
          folders: { ...s.folders, page: { ...s.folders.page, total } },
        })),
      setFoldersPageSize: (pageSize) =>
        set((s) => ({
          folders: { ...s.folders, page: { ...s.folders.page, pageSize } },
        })),

      library: { path: null, last_error: null },
      setLibrary: (library) => set({ library }),

      loading: false,
      setLoading: (loading) => set({ loading }),

      scanning: 0,
      bumpScanning: (delta) => set((s) => ({ scanning: s.scanning + delta })),

      refreshTick: 0,
      bumpRefreshTick: () =>
        set((s) => ({ refreshTick: s.refreshTick + 1 })),

      bumpTracksRefresh: () =>
        set((s) => ({ refreshTick: s.refreshTick + 1 })),

      tracksList: [],
      setTracksList: (tracksList) => set({ tracksList }),
      albumsList: [],
      setAlbumsList: (albumsList) => set({ albumsList }),
      genresList: [],
      setGenresList: (genresList) => set({ genresList }),
      yearsList: [],
      setYearsList: (yearsList) => set({ yearsList }),
      artistsList: [],
      setArtistsList: (artistsList) => set({ artistsList }),
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
    {
      name: "mimir.ui",
      version: 2,
      // Persist only the page state we want to restore across app
      // restarts. Runtime data (rows, library status, etc.) is always
      // recomputed on next mount. v2 invalidates the v1 payload (which
      // was stored with a shape that replaced the entire view slot on
      // rehydrate and crashed the page-fetch hook).
      partialize: pageOnlySlice,
      merge: mergePersisted,
    },
  ),
);

/** Look up the per-view page state in one place. Returns null for
 *  views that don't paginate (queue) or to keep callers honest. */
export function selectPage(s: Store, v: ViewKey): PageState | null {
  switch (v) {
    case "tracks":
      return s.tracks.page;
    case "albums":
      return s.albums.page;
    case "artists":
      return s.artists.page;
    case "genres":
      return s.genres.page;
    case "years":
      return s.years.page;
    case "folders":
      return s.folders.page;
    default:
      return null;
  }
}
