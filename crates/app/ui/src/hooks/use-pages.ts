// Per-view pagination fetches. Each hook runs in two effects:
//   1. (re)count whenever the view's input dimensions change — page
//      size, filter, query. The count clamps the saved `page` to the
//      valid range.
//   2. (re)fetch the rows for the *current clamped* page.
// Both effects write through the Zustand store so the views stay
// declarative — they just read `useStore((s) => s.tracksList)`.

import React, { useEffect } from "react";
import * as ipc from "@/lib/ipc";
import { useStore } from "@/lib/store";

/** Tracks view. Filter / query / search changes reset the page to 1;
 *  page / pageSize changes just refetch the current slice. */
export function useTracksPage() {
  const filter = useStore((s) => s.tracks.filter);
  const query = useStore((s) => s.tracks.query) ?? "";
  const page = useStore((s) => s.tracks.page.page);
  const pageSize = useStore((s) => s.tracks.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setTracksList = useStore((s) => s.setTracksList);
  const setTracksTotal = useStore((s) => s.setTracksTotal);
  const setTracksPage = useStore((s) => s.setTracksPage);
  const setLoading = useStore((s) => s.setLoading);

  const trimmed = query.trim();

  // Reset to page 1 on filter/query changes.
  useEffect(() => {
    setTracksPage(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    filter.genre ?? null,
    filter.year ?? null,
    filter.artistId ?? null,
    filter.albumId ?? null,
    trimmed,
  ]);

  // Count + (clamp) + fetch rows whenever the inputs change.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    const run = async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        let rows;
        let total;

        const hasFilter =
          filter.genre != null ||
          filter.year != null ||
          filter.artistId != null ||
          filter.albumId != null;

        if (hasFilter) {
          const [r, t] = await Promise.all([
            ipc.libraryQueryTracks({
              genre: filter.genre,
              year: filter.year,
              artistId: filter.artistId,
              albumId: filter.albumId,
              limit: pageSize,
              offset,
            }),
            ipc.libraryCountTracksFiltered({
              genre: filter.genre,
              year: filter.year,
              artistId: filter.artistId,
              albumId: filter.albumId,
            }),
          ]);
          rows = r;
          total = t;
        } else if (trimmed.length > 0) {
          const page_data = await ipc.librarySearchTracksPage(
            trimmed,
            pageSize,
            offset,
          );
          rows = page_data.rows;
          total = page_data.total;
        } else {
          const [r, t] = await Promise.all([
            ipc.libraryListTracks(pageSize, offset),
            ipc.libraryCountTracks(),
          ]);
          rows = r;
          total = t;
        }

        if (cancelled) return;
        setTracksTotal(total);
        const lastPage = Math.max(1, Math.ceil(total / pageSize));
        if (page > lastPage) {
          setTracksPage(lastPage);
        }
        setTracksList(rows);
      } catch (e) {
        console.error("useTracksPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    filter.genre ?? null,
    filter.year ?? null,
    filter.artistId ?? null,
    filter.albumId ?? null,
    trimmed,
    page,
    pageSize,
    refreshTick,
  ]);
}

export function useAlbumsPage() {
  const page = useStore((s) => s.albums.page.page);
  const pageSize = useStore((s) => s.albums.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setAlbumsList = useStore((s) => s.setAlbumsList);
  const setAlbumsTotal = useStore((s) => s.setAlbumsTotal);
  const setAlbumsPage = useStore((s) => s.setAlbumsPage);
  const setLoading = useStore((s) => s.setLoading);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [rows, total] = await Promise.all([
          ipc.libraryListAlbums(pageSize, offset),
          ipc.libraryCountAlbums(),
        ]);
        if (cancelled) return;
        setAlbumsTotal(total);
        const last = Math.max(1, Math.ceil(total / pageSize));
        if (page > last) setAlbumsPage(last);
        setAlbumsList(rows);
      } catch (e) {
        console.error("useAlbumsPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    page,
    pageSize,
    refreshTick,
    setAlbumsList,
    setAlbumsTotal,
    setAlbumsPage,
    setLoading,
  ]);
}

export function useArtistsPage() {
  const page = useStore((s) => s.artists.page.page);
  const pageSize = useStore((s) => s.artists.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setArtistsList = useStore((s) => s.setArtistsList);
  const setArtistsTotal = useStore((s) => s.setArtistsTotal);
  const setArtistsPage = useStore((s) => s.setArtistsPage);
  const setLoading = useStore((s) => s.setLoading);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [rows, total] = await Promise.all([
          ipc.libraryListArtists(pageSize, offset),
          ipc.libraryCountArtists(),
        ]);
        if (cancelled) return;
        setArtistsTotal(total);
        const last = Math.max(1, Math.ceil(total / pageSize));
        if (page > last) setArtistsPage(last);
        setArtistsList(rows);
      } catch (e) {
        console.error("useArtistsPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    page,
    pageSize,
    refreshTick,
    setArtistsList,
    setArtistsTotal,
    setArtistsPage,
    setLoading,
  ]);
}

export function useGenresPage() {
  const page = useStore((s) => s.genres.page.page);
  const pageSize = useStore((s) => s.genres.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setGenresList = useStore((s) => s.setGenresList);
  const setGenresTotal = useStore((s) => s.setGenresTotal);
  const setGenresPage = useStore((s) => s.setGenresPage);
  const setLoading = useStore((s) => s.setLoading);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [rows, total] = await Promise.all([
          ipc.libraryListGenres(pageSize, offset),
          ipc.libraryCountGenres(),
        ]);
        if (cancelled) return;
        setGenresTotal(total);
        const last = Math.max(1, Math.ceil(total / pageSize));
        if (page > last) setGenresPage(last);
        setGenresList(rows);
      } catch (e) {
        console.error("useGenresPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    page,
    pageSize,
    refreshTick,
    setGenresList,
    setGenresTotal,
    setGenresPage,
    setLoading,
  ]);
}

export function useYearsPage() {
  const page = useStore((s) => s.years.page.page);
  const pageSize = useStore((s) => s.years.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setYearsList = useStore((s) => s.setYearsList);
  const setYearsTotal = useStore((s) => s.setYearsTotal);
  const setYearsPage = useStore((s) => s.setYearsPage);
  const setLoading = useStore((s) => s.setLoading);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [rows, total] = await Promise.all([
          ipc.libraryListYears(pageSize, offset),
          ipc.libraryCountYears(),
        ]);
        if (cancelled) return;
        setYearsTotal(total);
        const last = Math.max(1, Math.ceil(total / pageSize));
        if (page > last) setYearsPage(last);
        setYearsList(rows);
      } catch (e) {
        console.error("useYearsPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    page,
    pageSize,
    refreshTick,
    setYearsList,
    setYearsTotal,
    setYearsPage,
    setLoading,
  ]);
}

/** Folders icons-mode: pages the files inside `folderId`. Returns
 *  the rows for the current page + the total. Tree mode does not call
 *  this. */
export function useFolderFilesPage(
  folderId: number | null,
): { rows: import("@/lib/types").FolderFile[]; total: number; loading: boolean } {
  const page = useStore((s) => s.folders.page.page);
  const pageSize = useStore((s) => s.folders.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setFoldersTotal = useStore((s) => s.setFoldersTotal);
  const setFoldersPage = useStore((s) => s.setFoldersPage);
  const setLoading = useStore((s) => s.setLoading);
  const [rows, setRows] = React.useState<import("@/lib/types").FolderFile[]>([]);
  const [total, setTotal] = React.useState(0);

  useEffect(() => {
    if (folderId == null) {
      setRows([]);
      setTotal(0);
      return;
    }
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [r, t] = await Promise.all([
          ipc.libraryListFolderFiles(folderId, pageSize, offset),
          ipc.libraryCountFolderFiles(folderId),
        ]);
        if (cancelled) return;
        setRows(r);
        setTotal(t);
        setFoldersTotal(t);
        const last = Math.max(1, Math.ceil(t / pageSize));
        if (page > last) setFoldersPage(last);
      } catch (e) {
        console.error("useFolderFilesPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    folderId,
    page,
    pageSize,
    refreshTick,
    setFoldersTotal,
    setFoldersPage,
    setLoading,
  ]);

  return { rows, total, loading: false };
}

/** Drill-down detail (album / genre / year / artist): pages the tracks
 *  matching `filter`. Writes to `tracksList` / `tracks.page` so the
 *  detail view's own pagination bar can drive it. */
export function useFilteredTracksPage(
  filter: Partial<import("@/lib/store").TracksFilter>,
) {
  const genre = filter.genre ?? null;
  const year = filter.year ?? null;
  const artistId = filter.artistId ?? null;
  const albumId = filter.albumId ?? null;
  const setTracksList = useStore((s) => s.setTracksList);
  const page = useStore((s) => s.tracks.page.page);
  const pageSize = useStore((s) => s.tracks.page.pageSize);
  const refreshTick = useStore((s) => s.refreshTick);
  const setTracksTotal = useStore((s) => s.setTracksTotal);
  const setTracksPage = useStore((s) => s.setTracksPage);
  const setLoading = useStore((s) => s.setLoading);

  // Reset to page 1 when the filter changes.
  useEffect(() => {
    setTracksPage(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [genre, year, artistId, albumId]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const offset = Math.max(0, (page - 1) * pageSize);
        const [rows, total] = await Promise.all([
          ipc.libraryQueryTracks({
            genre,
            year,
            artistId,
            albumId,
            limit: pageSize,
            offset,
          }),
          ipc.libraryCountTracksFiltered({ genre, year, artistId, albumId }),
        ]);
        if (cancelled) return;
        setTracksTotal(total);
        const last = Math.max(1, Math.ceil(total / pageSize));
        if (page > last) setTracksPage(last);
        setTracksList(rows);
      } catch (e) {
        console.error("useFilteredTracksPage fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    genre,
    year,
    artistId,
    albumId,
    page,
    pageSize,
    refreshTick,
    setTracksList,
    setTracksTotal,
    setTracksPage,
    setLoading,
  ]);
}
