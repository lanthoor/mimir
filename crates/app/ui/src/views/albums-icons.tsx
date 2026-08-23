import { Card } from "@/components/ui/card";
import { AlbumCover } from "@/components/album-cover";
import { useStore } from "@/lib/store";

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
        <Card
          key={a.id}
          className="cursor-pointer overflow-hidden transition-colors hover:border-primary"
          onDoubleClick={() => selectAlbum(a.id)}
        >
          <AlbumCover albumId={a.id} className="aspect-square w-full" />
            <div className="p-3">
              <div className="truncate font-semibold">{a.title}</div>
              <div className="truncate text-xs text-muted-foreground">
                {[a.artist_name, `${a.track_count} tracks`]
                  .filter(Boolean)
                  .join(" — ")}
              </div>
            </div>
        </Card>
      ))}
    </div>
  );
}
