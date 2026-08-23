import { useEffect, useState } from "react";
import { ArrowLeft, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { AlbumCover } from "@/components/album-cover";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import { AlbumTracksTable } from "@/views/album-tracks-table";
import { addAlbumToQueue } from "@/lib/queue-actions";
import type { TrackRow } from "@/lib/types";

type Props = { albumId: number };

export function AlbumDetail({ albumId }: Props) {
  const selectAlbum = useStore((s) => s.selectAlbum);
  const album = useStore((s) =>
    s.albumsList.find((a) => a.id === albumId),
  );
  const [tracks, setTracks] = useState<TrackRow[] | null>(null);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  useEffect(() => {
    if (!album) {
      selectAlbum(null);
      return;
    }
    setTracks(null);
    ipc
      .libraryQueryTracks({ albumId: album.id, limit: 1000 })
      .then((t) => setTracks(t))
      .catch(console.error);
  }, [album, selectAlbum]);

  if (!album) {
    return (
      <div className="p-6 text-sm text-muted-foreground">
        Album not found.
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-4">
        <Button
          variant="outline"
          size="sm"
          onClick={() => selectAlbum(null)}
        >
          <ArrowLeft />
          Albums
        </Button>
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
      {tracks == null ? (
        <div className="p-6 text-sm text-muted-foreground">Loading…</div>
      ) : tracks.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">
          No tracks indexed.
        </div>
      ) : (
        <AlbumTracksTable tracks={tracks} />
      )}
    </div>
  );
}
