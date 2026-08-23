import { useState } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useStore } from "@/lib/store";
import { formatDuration, basename } from "@/lib/utils";
import * as ipc from "@/lib/ipc";
import { EditTrackDialog } from "@/components/edit-track-dialog";
import { TrackContextMenu } from "@/components/track-context-menu";
import type { TrackRow } from "@/lib/types";

export function AlbumTracksTable({ tracks }: { tracks: TrackRow[] }) {
  const [editing, setEditing] = useState<number | null>(null);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  return (
    <>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-12 text-right">#</TableHead>
            <TableHead>Title</TableHead>
            <TableHead>Artist</TableHead>
            <TableHead>Genre</TableHead>
            <TableHead className="w-20 text-right">Time</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {tracks.map((t) => (
            <TrackContextMenu
              key={t.id}
              trackId={t.id}
              trackTitle={t.title ?? basename(t.path)}
              trackArtist={t.artist_name ?? ""}
              onEdit={() => setEditing(t.id)}
            >
              <TableRow
                className="cursor-pointer"
                onDoubleClick={() => {
                  setNowPlaying(t.title ?? "(untitled)", t.artist_name ?? "");
                  setNowPlayingTrackId(t.id);
                  setLyricsTrackId(t.id);
                  ipc.audioPlay(t.id).catch(console.error);
                }}
              >
                <TableCell className="w-12 text-right text-muted-foreground tabular-nums">
                  {t.track_no ?? ""}
                </TableCell>
                <TableCell className="font-medium">
                  {t.title ?? "(untitled)"}
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {t.artist_name ?? ""}
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {t.genre ?? ""}
                </TableCell>
                <TableCell className="w-20 text-right text-muted-foreground tabular-nums">
                  {formatDuration(t.duration_ms)}
                </TableCell>
              </TableRow>
            </TrackContextMenu>
          ))}
        </TableBody>
      </Table>
      {editing != null && (
        <EditTrackDialog
          trackId={editing}
          open={editing != null}
          onOpenChange={(o) => {
            if (!o) setEditing(null);
          }}
        />
      )}
    </>
  );
}
