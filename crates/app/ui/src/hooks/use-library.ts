// Library refresh + status. All IPC errors bubble up to the caller so
// views can surface them.

import { useCallback } from "react";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import { useStore } from "@/lib/store";

export function useLibrary() {
  const view = useStore((s) => s.view);
  const tracksFilter = useStore((s) => s.tracks.filter);
  const tracksQuery = useStore((s) => s.tracks.query);
  const setLoading = useStore((s) => s.setLoading);
  const setLibrary = useStore((s) => s.setLibrary);
  const setTracksList = useStore((s) => s.setTracksList);
  const setAlbumsList = useStore((s) => s.setAlbumsList);
  const setGenresList = useStore((s) => s.setGenresList);
  const setYearsList = useStore((s) => s.setYearsList);
  const setArtistsList = useStore((s) => s.setArtistsList);
  const setFolderTree = useStore((s) => s.setFolderTree);
  const setCwd = useStore((s) => s.setCwd);

  const refreshStatus = useCallback(async () => {
    try {
      const status = await ipc.libraryStatus();
      setLibrary(status);
    } catch (e) {
      console.error("library_status failed:", e);
      setLibrary({ path: null, last_error: (e as Error).message });
    }
  }, [setLibrary]);

  const refreshAlbums = useCallback(async () => {
    const albums = await ipc.libraryListAlbums(200, 0);
    setAlbumsList(albums);
    // Covers are not fetched here — each `<img>` points at the host's
    // `mimircover://localhost/cover/{id}` protocol URL and the webview
    // loads + caches them natively. (The old design pulled every cover's
    // bytes over IPC and turned them into data: URLs; depending the
    // refresh callback on that cache also created an infinite
    // re-fetch loop that froze the main thread until the OS killed the
    // window.)
  }, [setAlbumsList]);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      if (view === "tracks") {
        const filter = tracksFilter;
        const hasFilter = Object.values(filter).some((v) => v != null);
        const q = tracksQuery.trim();
        let rows;
        if (hasFilter) {
          rows = await ipc.libraryQueryTracks({
            genre: filter.genre,
            year: filter.year,
            artistId: filter.artistId,
            albumId: filter.albumId,
          });
        } else if (q.length > 0) {
          rows = await ipc.librarySearch(q, 100);
        } else {
          rows = await ipc.libraryListTracks(100, 0);
        }
        setTracksList(rows);
      } else if (view === "albums") {
        await refreshAlbums();
      } else if (view === "genres") {
        setGenresList(await ipc.libraryListGenres());
      } else if (view === "years") {
        setYearsList(await ipc.libraryListYears());
      } else if (view === "artists") {
        setArtistsList(await ipc.libraryListArtists());
      } else if (view === "folders") {
        const tree = await ipc.libraryFolderTree();
        setFolderTree(tree);
        // Drop cwd if it no longer resolves.
        const cwd = useStore.getState().folders.cwd;
        if (cwd != null) {
          const stillThere = findByPath(tree.root_children, cwd);
          if (!stillThere) setCwd(null);
        }
      }
    } catch (e) {
      console.error("refresh failed:", e);
      toast.error(`Refresh failed: ${describeError(e)}`);
    } finally {
      setLoading(false);
    }
  }, [
    view,
    tracksFilter,
    tracksQuery,
    setLoading,
    setTracksList,
    setGenresList,
    setYearsList,
    setArtistsList,
    setFolderTree,
    setCwd,
    refreshAlbums,
  ]);

  return { refresh, refreshStatus, refreshAlbums };
}

function findByPath(
  nodes: import("@/lib/types").FolderNode[],
  path: string,
): import("@/lib/types").FolderNode | null {
  for (const n of nodes) {
    if (n.path === path) return n;
    const inChild = findByPath(n.children, path);
    if (inChild) return inChild;
  }
  return null;
}

function describeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}
