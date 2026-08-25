import { useStore } from "@/lib/store";
import { AlbumsIcons } from "@/views/albums-icons";
import { AlbumsList } from "@/views/albums-list";
import { AlbumDetail } from "@/views/album-detail";
import { ViewToolbar } from "@/views/view-toolbar";
import { PaginationBar } from "@/components/pagination-bar";

export function AlbumsView() {
  const mode = useStore((s) => s.albums.mode);
  const selectedAlbumId = useStore((s) => s.albums.selectedAlbumId);
  const page = useStore((s) => s.albums.page);
  const setAlbumsPage = useStore((s) => s.setAlbumsPage);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <ViewToolbar view="albums" />
      {selectedAlbumId != null ? (
        <AlbumDetail albumId={selectedAlbumId} />
      ) : mode === "list" ? (
        <AlbumsList />
      ) : (
        <AlbumsIcons />
      )}
      <PaginationBar
        page={page.page}
        pageSize={page.pageSize}
        total={page.total}
        onChange={setAlbumsPage}
      />
    </div>
  );
}
