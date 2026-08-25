import { useRef, useState } from "react";
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
import { useVisibleFit } from "@/hooks/use-visible-fit";

export function TracksList() {
  const items = useStore((s) => s.tracksList);
  const setTracksPageSize = useStore((s) => s.setTracksPageSize);
  const [editing, setEditing] = useState<number | null>(null);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  const containerRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLTableRowElement>(null);

  // Tables are single-column; pageSize == rowsVisible minus the header.
  const fit = useVisibleFit({
    containerRef,
    probeRef,
    minCellWidth: 36,
    columns: 1,
    gap: 0,
    headerRows: 1,
    onChange: setTracksPageSize,
  });

  return (
    <div
      ref={containerRef}
      className="flex-1 overflow-hidden"
    >
      {items.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">No tracks.</div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12 text-right">#</TableHead>
              <TableHead>Title</TableHead>
              <TableHead>Artist</TableHead>
              <TableHead>Album</TableHead>
              <TableHead>Genre</TableHead>
              <TableHead className="w-16 text-right">Year</TableHead>
              <TableHead className="w-20 text-right">Time</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.map((t, i) => (
              <TrackContextMenu
                key={t.id}
                trackId={t.id}
                trackTitle={t.title ?? basename(t.path)}
                trackArtist={t.artist_name ?? ""}
                onEdit={() => setEditing(t.id)}
              >
                <TableRow
                  ref={i === 0 ? probeRef : null}
                  onDoubleClick={() => {
                    setNowPlaying(
                      t.title ?? "(untitled)",
                      t.artist_name ?? "",
                    );
                    setNowPlayingTrackId(t.id);
                    setLyricsTrackId(t.id);
                    ipc.audioPlay(t.id).catch(console.error);
                  }}
                  className="cursor-pointer"
                >
                  <TableCell className="w-12 text-right text-muted-foreground tabular-nums">
                    {t.track_no ?? ""}
                  </TableCell>
                  <TableCell className="font-medium">
                    {t.title ?? "(untitled)"}
                  </TableCell>
                  <TableCell>{t.artist_name ?? ""}</TableCell>
                  <TableCell>{t.album_title ?? ""}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {t.genre ?? ""}
                  </TableCell>
                  <TableCell className="w-16 text-right text-muted-foreground tabular-nums">
                    {t.year ?? ""}
                  </TableCell>
                  <TableCell className="w-20 text-right text-muted-foreground tabular-nums">
                    {formatDuration(t.duration_ms)}
                  </TableCell>
                </TableRow>
              </TrackContextMenu>
            ))}
            {Array.from({
              length: Math.max(0, fit.pageSize - items.length - 1), // -1: header row
            }).map((_, i) => (
              <TableRow key={`pad-${i}`} aria-hidden>
                <TableCell colSpan={7} className="h-9" />
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {editing != null && (
        <EditTrackDialog
          trackId={editing}
          open={editing != null}
          onOpenChange={(o) => {
            if (!o) setEditing(null);
          }}
        />
      )}
    </div>
  );
}
