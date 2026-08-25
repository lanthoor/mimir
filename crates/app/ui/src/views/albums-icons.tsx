import { useRef } from "react";
import { Card } from "@/components/ui/card";
import { AlbumCover } from "@/components/album-cover";
import { useStore } from "@/lib/store";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useAlbumsPage } from "@/hooks/use-pages";
import { useVisibleFit } from "@/hooks/use-visible-fit";
import { addAlbumToQueue } from "@/lib/queue-actions";

export function AlbumsIcons() {
  useAlbumsPage();
  const items = useStore((s) => s.albumsList);
  const selectAlbum = useStore((s) => s.selectAlbum);
  const setAlbumsPageSize = useStore((s) => s.setAlbumsPageSize);

  const containerRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);

  const fit = useVisibleFit({
    containerRef,
    gridRef,
    probeRef,
    minCellWidth: 180,
    gap: 12,
    onChange: setAlbumsPageSize,
  });

  return (
    <div ref={containerRef} className="flex-1 overflow-hidden">
      {items.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">No albums.</div>
      ) : (
        <div
          ref={gridRef}
          className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3"
        >
          {items.map((a, i) => (
            <ContextMenu key={a.id}>
              <ContextMenuTrigger asChild>
                <Card
                  ref={i === 0 ? probeRef : null}
                  className="cursor-pointer overflow-hidden transition-colors hover:border-primary"
                  onDoubleClick={() => selectAlbum(a.id)}
                >
                  <AlbumCover
                    albumId={a.id}
                    className="aspect-square w-full"
                  />
                  <div className="p-3">
                    <div className="break-words font-semibold">{a.title}</div>
                    <div className="break-words text-xs text-muted-foreground">
                      {[a.artist_name, `${a.track_count} tracks`]
                        .filter(Boolean)
                        .join(" — ")}
                    </div>
                  </div>
                </Card>
              </ContextMenuTrigger>
              <ContextMenuContent>
                <ContextMenuItem onClick={() => selectAlbum(a.id)}>
                  Open
                </ContextMenuItem>
                <ContextMenuItem
                  onClick={() => void addAlbumToQueue(a.id, a.title)}
                >
                  Add album to queue
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          ))}
          {Array.from({
            length: Math.max(0, fit.pageSize - items.length),
          }).map((_, i) => (
            <div
              key={`pad-${i}`}
              aria-hidden
              className="invisible rounded-md border border-transparent"
              style={{ height: fit.rowHeight || undefined }}
              data-placeholder
            />
          ))}
        </div>
      )}
    </div>
  );
}
