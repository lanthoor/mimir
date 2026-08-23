import type { ReactNode } from "react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import * as ipc from "@/lib/ipc";
import { useStore } from "@/lib/store";
import { addTracksToQueue } from "@/lib/queue-actions";

type Props = {
  children: ReactNode;
  trackId: number;
  trackTitle: string;
  trackArtist: string;
  onEdit: () => void;
};

export function TrackContextMenu({
  children,
  trackId,
  trackTitle,
  trackArtist,
  onEdit,
}: Props) {
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem
          onClick={() => {
            setNowPlaying(trackTitle, trackArtist);
            setNowPlayingTrackId(trackId);
            setLyricsTrackId(trackId);
          ipc.audioPlay(trackId).catch(console.error);
        }}
      >
        Play
      </ContextMenuItem>
      <ContextMenuItem onClick={() => void addTracksToQueue([trackId])}>
        Add to queue
      </ContextMenuItem>
      <ContextMenuItem onClick={onEdit}>Edit…</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
