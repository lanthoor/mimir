import { useCallback, useEffect, useState } from "react";
import { GripVertical, ListMusic, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import type { QueueItem } from "@/lib/types";

/**
 * The live playback queue — a persistent, circular playlist. The currently
 * playing track is marked (`is_current`) and can appear on any row; the
 * list does not shrink as tracks finish, it wraps around from the last to
 * the first.
 *
 * Reorder with native HTML5 drag-and-drop (no dnd library — it's a flat
 * list). The currently playing row is not draggable; every other row can be
 * dragged to any slot (double-click a row to jump to it; "×" removes it).
 */
export function QueueView() {
  const [items, setItems] = useState<QueueItem[]>([]);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [over, setOver] = useState<{ index: number; before: boolean } | null>(null);

  const refresh = useCallback(async () => {
    try {
      setItems(await ipc.audioQueueGet());
    } catch (e) {
      console.error("queue_get failed", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = setInterval(() => void refresh(), 1500);
    return () => clearInterval(id);
  }, [refresh]);

  const removeAt = useCallback(
    async (index: number) => {
      await ipc
        .audioQueueRemoveAt(index)
        .then(refresh)
        .catch((e) => toast.error("Remove failed", { description: String(e) }));
    },
    [refresh],
  );

  const playAt = useCallback(
    (index: number) => {
      ipc
        .audioQueuePlayAt(index)
        .then(refresh)
        .catch((e) => toast.error("Play failed", { description: String(e) }));
    },
    [refresh],
  );

  const clear = useCallback(async () => {
    if (!window.confirm("Clear the queue and stop playback?")) return;
    await ipc
      .audioQueueClear()
      .then(refresh)
      .catch((e) => toast.error("Clear failed", { description: String(e) }));
  }, [refresh]);

  const onDragStart = (index: number) => (e: React.DragEvent<HTMLLIElement>) => {
    e.dataTransfer.setData("text/plain", String(index));
    e.dataTransfer.effectAllowed = "move";
    setDragIndex(index);
  };

  const onDragOver = (index: number) => (e: React.DragEvent<HTMLLIElement>) => {
    if (dragIndex == null) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    const rect = e.currentTarget.getBoundingClientRect();
    const before = e.clientY < rect.top + rect.height / 2;
    setOver({ index, before });
  };

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    const done = () => {
      setDragIndex(null);
      setOver(null);
    };
    if (dragIndex == null || over == null) {
      done();
      return;
    }
    // Insert position in row numbers: before `over.index` → over.index,
    // after → over.index + 1. Clamp to a valid slot.
    let to = over.before ? over.index : over.index + 1;
    to = Math.max(0, Math.min(to, items.length - 1));
    // Drop on the dragged track's own slot → no-op.
    if (to === dragIndex) {
      done();
      return;
    }
    ipc
      .audioQueueMove(dragIndex, to)
      .then(refresh)
      .catch((e) => toast.error("Reorder failed", { description: String(e) }))
      .finally(done);
  };

  // Which row should show the insertion line (its top border)?
  const target: number | null =
    over != null ? (over.before ? over.index : over.index + 1) : null;

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <ListMusic className="h-5 w-5 text-muted-foreground" />
        <h2 className="text-lg font-semibold">Queue</h2>
        <span className="text-sm text-muted-foreground">
          {items.length} track{items.length === 1 ? "" : "s"}
        </span>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" onClick={clear} disabled={items.length === 0}>
            Clear all
          </Button>
        </div>
      </div>
      <div
        className="flex-1 overflow-auto rounded-md border border-border"
        onDragOver={(e) => e.preventDefault()}
        onDrop={onDrop}
        onDragLeave={() => setOver(null)}
      >
        {items.length === 0 ? (
          <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
            Queue is empty. Right-click a track, album, or folder and choose “Add
            to queue” — double-click a listed track to jump to it.
          </div>
        ) : (
          <ol className="divide-y divide-border">
            {items.map((item) => (
              <li
                key={item.track_id}
                draggable={!item.is_current}
                onDragStart={item.is_current ? undefined : onDragStart(item.index)}
                onDragOver={item.is_current ? undefined : onDragOver(item.index)}
                onDoubleClick={() => playAt(item.index)}
                className={
                  "group flex cursor-default items-center gap-2 px-2 py-1.5 text-sm " +
                  (item.is_current ? "bg-primary/5" : "") +
                  (dragIndex === item.index ? " opacity-50" : "") +
                  (target === item.index ? " border-t-2 border-primary" : "")
                }
              >
                <span className="w-8 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
                  {item.index + 1}
                </span>
                {item.is_current ? null : (
                  <GripVertical className="h-4 w-4 shrink-0 cursor-grab text-muted-foreground" />
                )}
                <div className="flex min-w-0 flex-1 flex-col leading-tight">
                  <span className={"truncate " + (item.is_current ? "font-semibold" : "")}>
                    {item.title}
                  </span>
                  {item.artist_name && (
                    <span className="truncate text-xs text-muted-foreground">
                      {item.artist_name}
                    </span>
                  )}
                </div>
                {item.is_current ? (
                  <span className="ml-2 shrink-0 text-xs font-medium text-primary">
                    playing
                  </span>
                ) : (
                  <button
                    type="button"
                    className="shrink-0 rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:bg-accent group-hover:opacity-100"
                    title="Remove from queue"
                    onClick={(e) => {
                      e.stopPropagation();
                      void removeAt(item.index);
                    }}
                  >
                    <X className="h-4 w-4" />
                  </button>
                )}
              </li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
