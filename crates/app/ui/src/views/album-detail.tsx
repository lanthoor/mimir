import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { AlbumCover } from "@/components/album-cover";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import { AlbumTracksTable } from "@/views/album-tracks-table";
import { PaginationBar } from "@/components/pagination-bar";
import { addAlbumToQueue } from "@/lib/queue-actions";
import { useFilteredTracksPage } from "@/hooks/use-pages";

type Props = { albumId: number };

export function AlbumDetail({ albumId }: Props) {
  useFilteredTracksPage({ albumId });
  const selectAlbum = useStore((s) => s.selectAlbum);
  const album = useStore((s) =>
    s.albumsList.find((a) => a.id === albumId),
  );
  const tracks = useStore((s) => s.tracksList);
  const total = useStore((s) => s.tracks.page.total);
  const page = useStore((s) => s.tracks.page.page);
  const pageSize = useStore((s) => s.tracks.page.pageSize);
  const setTracksPage = useStore((s) => s.setTracksPage);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  if (!album) {
    return (
      <div className="p-6 text-sm text-muted-foreground">
        Album not found.
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col gap-4">
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbLink asChild>
              <button type="button" onClick={() => selectAlbum(null)}>
                Albums
              </button>
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>{album.title}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div className="flex items-center gap-4">
        <AlbumCover albumId={album.id} className="h-28 w-28 rounded-md" />
        <div className="flex min-w-0 flex-col">
          <div className="text-2xl font-bold">{album.title}</div>
          <div className="text-muted-foreground">
            {album.artist_name ?? "Unknown artist"}
          </div>
          <div className="mt-1 flex gap-2">
            <Badge variant="secondary">{album.track_count} tracks</Badge>
            {album.year != null && (
              <Badge variant="outline">{album.year}</Badge>
            )}
          </div>
        </div>
        <div className="ml-auto flex gap-2">
          <Button
            variant="default"
            size="sm"
            onClick={() => {
              if (!tracks || tracks.length === 0) return;
              const first = tracks[0];
              setNowPlaying(
                first.title ?? "(untitled)",
                first.artist_name ?? album.artist_name ?? "",
              );
              setNowPlayingTrackId(first.id);
              setLyricsTrackId(first.id);
              void ipc.audioPlayAndEnqueueMany(tracks.map((t) => t.id));
            }}
            disabled={!tracks || tracks.length === 0}
          >
            <Play />
            Play
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              if (!tracks || tracks.length === 0) return;
              void addAlbumToQueue(album.id, album.title);
            }}
            disabled={!tracks || tracks.length === 0}
          >
            Add to queue
          </Button>
        </div>
      </div>
      {tracks.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">
          No tracks indexed.
        </div>
      ) : (
        <AlbumTracksTable tracks={tracks} />
      )}
      <PaginationBar
        page={page}
        pageSize={pageSize}
        total={total}
        onChange={setTracksPage}
      />
    </div>
  );
}
