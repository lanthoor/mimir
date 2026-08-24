import { Music } from "lucide-react";
import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import { EditTrackDialog } from "@/components/edit-track-dialog";
import { useState } from "react";
import { TrackContextMenu } from "@/components/track-context-menu";
import { basename } from "@/lib/utils";

export function TracksIcons() {
  const items = useStore((s) => s.tracksList);
  if (items.length === 0) {
    return (
      <div className="p-6 text-sm text-muted-foreground">No tracks.</div>
    );
  }
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
      {items.map((t) => (
        <TrackIcon key={t.id} track={t} />
      ))}
    </div>
  );
}

function TrackIcon({ track }: { track: import("@/lib/types").TrackRow }) {
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
