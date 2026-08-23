// Shared "add to queue" actions for tracks, albums and folders.
// Kept in one place so every context menu / button does the same thing.

import { toast } from "sonner";
import * as ipc from "./ipc";
import type { FolderNode } from "./types";

/** Append `trackIds` to the end of the queue. */
export async function addTracksToQueue(
  trackIds: number[],
  label?: string,
): Promise<void> {
  if (trackIds.length === 0) {
    toast.warning("Nothing to add");
    return;
  }
  try {
    const appended = await ipc.audioQueueEnqueueMany(trackIds);
    toast.success(
      `${appended.length} track${appended.length === 1 ? "" : "s"} added to queue${label ? ` — ${label}` : ""}`,
    );
  } catch (e) {
    toast.error(`Add to queue failed: ${describe(e)}`);
  }
}

/** Resolve an album's track ids and add them to the queue. */
export async function addAlbumToQueue(albumId: number, title?: string): Promise<void> {
  try {
    const tracks = await ipc.libraryQueryTracks({ albumId, limit: 10_000 });
    const ids = tracks.map((t) => t.id);
    await addTracksToQueue(ids, title);
  } catch (e) {
    toast.error(`Album add failed: ${describe(e)}`);
  }
}

/** Collect every indexed track id under a folder node (files + descendants). */
export function collectFolderTrackIds(node: FolderNode): number[] {
  const out: number[] = [];
  const walk = (n: FolderNode): void => {
    for (const f of n.files ?? []) {
      if (f.track_id != null) out.push(f.track_id);
    }
    for (const c of n.children ?? []) walk(c);
  };
  walk(node);
  return [...new Set(out)];
}

function describe(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}
