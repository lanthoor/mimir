// Library refresh + status. All IPC errors bubble up to the caller so
// views can surface them.
//
// The per-view pagination hooks (`use-pages.ts`) own rows/list fetching
// now. This hook is only left with status refresh and the folder tree
// fetch (which is consumed by the Folders view, not by pagination).

import { useCallback } from "react";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import { useStore } from "@/lib/store";

export function useLibrary() {
  const setLibrary = useStore((s) => s.setLibrary);
  const setLoading = useStore((s) => s.setLoading);
  const setFolderTree = useStore((s) => s.setFolderTree);

  const refreshStatus = useCallback(async () => {
    try {
      const status = await ipc.libraryStatus();
      setLibrary(status);
    } catch (e) {
      console.error("library_status failed:", e);
      setLibrary({ path: null, last_error: (e as Error).message });
    }
  }, [setLibrary]);

  // Folder tree refresh: file additions / removals shift the directory
  // layout, so the Folders view re-fetches its tree on scan complete.
  const refreshFolders = useCallback(async () => {
    setLoading(true);
    try {
      const tree = await ipc.libraryFolderTree();
      setFolderTree(tree);
    } catch (e) {
      console.error("library_folder_tree failed:", e);
      toast.error(`Folders refresh failed: ${describeError(e)}`);
    } finally {
      setLoading(false);
    }
  }, [setFolderTree, setLoading]);

  return { refreshStatus, refreshFolders };
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
