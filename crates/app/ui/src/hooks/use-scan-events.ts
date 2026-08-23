// Wires the Rust scan:done / scan:error events to a toast and a scan
// counter. The counter drives the indeterminate progress bar at the top
// of the shell.

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import { useStore } from "@/lib/store";
import { useLibrary } from "./use-library";
import type { ScanDonePayload, ScanErrorPayload } from "@/lib/types";

export function useScanEvents() {
  const { refreshAlbums, refresh } = useLibrary();

  useEffect(() => {
    const unlistens: UnlistenFn[] = [];
    let cancelled = false;

    (async () => {
      const u1 = await listen<ScanDonePayload>("scan:done", (e) => {
        const { path, summary } = e.payload;
        useStore.getState().bumpScanning(-1);
        useStore.getState().setLoading(false);
        toast.success(
          summariseScan(path, summary),
          { id: `scan-${path}` },
        );
        // Refresh whatever view is active so the new tracks show up.
        refresh().catch(console.error);
        refreshAlbums().catch(console.error);
      });
      if (cancelled) {
        u1();
        return;
      }
      unlistens.push(u1);

      const u2 = await listen<ScanErrorPayload>("scan:error", (e) => {
        const { path, error } = e.payload;
        useStore.getState().bumpScanning(-1);
        useStore.getState().setLoading(false);
        toast.error(`Add folder failed for ${path}: ${error}`);
      });
      if (cancelled) {
        u2();
        return;
      }
      unlistens.push(u2);
    })().catch(console.error);

    return () => {
      cancelled = true;
      for (const u of unlistens) u();
    };
  }, [refresh, refreshAlbums]);
}

function summariseScan(path: string, s: ScanDonePayload["summary"]): string {
  return `Added ${s.sent} new tracks from ${path} (skipped ${s.known} known, ${s.hashed_fail} unreadable).`;
}

export async function startFolderAdd(path: string): Promise<void> {
  useStore.getState().bumpScanning(1);
  useStore.getState().setLoading(true);
  try {
    await ipc.libraryAddFolder(path);
  } catch (e) {
    useStore.getState().bumpScanning(-1);
    useStore.getState().setLoading(false);
    throw e;
  }
}

export async function startFoldersAdd(paths: string[]): Promise<void> {
  for (const _ of paths) useStore.getState().bumpScanning(1);
  useStore.getState().setLoading(true);
  try {
    await ipc.libraryAddFolders(paths);
  } catch (e) {
    useStore.getState().bumpScanning(-paths.length);
    useStore.getState().setLoading(false);
    throw e;
  }
}
