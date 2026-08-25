import { useRef, useState } from "react";
import { Music } from "lucide-react";
import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import { EditTrackDialog } from "@/components/edit-track-dialog";
import { TrackContextMenu } from "@/components/track-context-menu";
import { basename } from "@/lib/utils";
import { useVisibleFit } from "@/hooks/use-visible-fit";

export function TracksIcons() {
  const items = useStore((s) => s.tracksList);
  const setTracksPageSize = useStore((s) => s.setTracksPageSize);

  const containerRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);

  const fit = useVisibleFit({
    containerRef,
    gridRef,
    probeRef,
    minCellWidth: 220,
    gap: 12,
    onChange: setTracksPageSize,
  });

  return (
    <div ref={containerRef} className="flex-1 overflow-hidden">
      {items.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">No tracks.</div>
      ) : (
        <div
          ref={gridRef}
          className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3"
        >
          {items.map((t, i) => (
            <TrackIcon
              key={t.id}
              track={t}
              probeRef={i === 0 ? probeRef : null}
            />
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

function TrackIcon({
  track,
  probeRef,
}: {
  track: import("@/lib/types").TrackRow;
  probeRef: React.Ref<HTMLDivElement> | null;
}) {
  const [editing, setEditing] = useState(false);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  return (
    <>
      <TrackContextMenu
        onEdit={() => setEditing(true)}
        trackId={track.id}
        trackTitle={track.title ?? basename(track.path)}
        trackArtist={track.artist_name ?? ""}
      >
        <Card
          ref={probeRef}
          className="flex cursor-pointer flex-col gap-1 p-3 transition-colors hover:border-primary"
          onDoubleClick={() => {
            setNowPlaying(track.title ?? "(untitled)", track.artist_name ?? "");
            setNowPlayingTrackId(track.id);
            setLyricsTrackId(track.id);
            ipc.audioPlay(track.id).catch(console.error);
          }}
        >
          <div className="flex items-center gap-2 text-base font-semibold">
            <Music className="h-4 w-4 shrink-0 text-muted-foreground" />
            <span className="break-words">{track.title ?? "(untitled)"}</span>
          </div>
          <div className="break-words text-xs text-muted-foreground">
            {[track.artist_name, track.album_title].filter(Boolean).join(" — ") ||
              track.path}
          </div>
        </Card>
      </TrackContextMenu>
      {editing && (
        <EditTrackDialog
          trackId={track.id}
          open={editing}
          onOpenChange={setEditing}
        />
      )}
    </>
  );
}
