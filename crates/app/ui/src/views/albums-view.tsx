import { useStore } from "@/lib/store";
import { AlbumsIcons } from "@/views/albums-icons";
import { AlbumsList } from "@/views/albums-list";
import { AlbumDetail } from "@/views/album-detail";
import { ViewToolbar } from "@/views/view-toolbar";

export function AlbumsView() {
  const mode = useStore((s) => s.albums.mode);
  const selectedAlbumId = useStore((s) => s.albums.selectedAlbumId);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <ViewToolbar view="albums" />
      <div className="flex-1 overflow-auto">
        {selectedAlbumId != null ? (
          <AlbumDetail albumId={selectedAlbumId} />
        ) : mode === "list" ? (
          <AlbumsList />
        ) : (
          <AlbumsIcons />
        )}
      </div>
    </div>
  );
}
