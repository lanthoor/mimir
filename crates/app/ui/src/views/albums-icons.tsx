import { Card } from "@/components/ui/card";
import { AlbumCover } from "@/components/album-cover";
import { useStore } from "@/lib/store";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { addAlbumToQueue } from "@/lib/queue-actions";

export function AlbumsIcons() {
  const items = useStore((s) => s.albumsList);
  const selectAlbum = useStore((s) => s.selectAlbum);

  if (items.length === 0) {
    return (
      <div className="p-6 text-sm text-muted-foreground">No albums.</div>
    );
  }

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
      {items.map((a) => (
        <ContextMenu key={a.id}>
          <ContextMenuTrigger asChild>
            <Card
              className="cursor-pointer overflow-hidden transition-colors hover:border-primary"
              onDoubleClick={() => selectAlbum(a.id)}
            >
              <AlbumCover albumId={a.id} className="aspect-square w-full" />
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
    </div>
  );
}
