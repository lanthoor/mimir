import { useCallback, useEffect } from "react";
import {
  SkipBack,
  Play,
  Pause,
  Square,
  SkipForward,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { LyricsDialog } from "@/components/lyrics-dialog";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import { useLibrary } from "@/hooks/use-library";

export function NowPlaying() {
  const nowPlayingTrackId = useStore((s) => s.nowPlayingTrackId);
  const nowPlayingTitle = useStore((s) => s.nowPlayingTitle);
  const nowPlayingArtist = useStore((s) => s.nowPlayingArtist);
  const lyricsTrackId = useStore((s) => s.lyricsTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);
  const playerSnapshot = useStore((s) => s.playerSnapshot);
  const setPlayerSnapshot = useStore((s) => s.setPlayerSnapshot);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const { refresh } = useLibrary();

  const play = useCallback(() => {
    if (nowPlayingTrackId != null) {
      ipc.audioPlay(nowPlayingTrackId).catch(console.error);
    }
  }, [nowPlayingTrackId]);
  const pause = useCallback(() => ipc.audioPause().catch(console.error), []);
  const stop = useCallback(() => ipc.audioStop().catch(console.error), []);
  const next = useCallback(() => ipc.audioNext().catch(console.error), []);
  const prev = useCallback(() => ipc.audioPrevious().catch(console.error), []);

  useEffect(() => {
    // After play, refresh the track list to update now-playing metadata.
    if (nowPlayingTrackId != null) {
      ipc
        .audioPlayerSnapshot()
        .then((snap) => {
          if (snap) setPlayerSnapshot(snap);
        })
        .catch(console.error);
    }
  }, [nowPlayingTrackId, setPlayerSnapshot]);

  useEffect(() => {
    // Pull fresh snapshot whenever the player changes state.
    const id = setInterval(() => {
      ipc
        .audioPlayerSnapshot()
        .then((snap) => setPlayerSnapshot(snap))
        .catch(() => {
          /* ignore */
        });
    }, 2000);
    return () => clearInterval(id);
  }, [setPlayerSnapshot]);

  // Sync the now-playing header from the player snapshot (path → title).
  useEffect(() => {
    if (playerSnapshot?.current && nowPlayingTrackId != null) {
      // The current path is the file path; we don't have title/artist
      // for it without another round-trip. The track list refresh from
      // `play_track` updates them via the caller.
      void refresh();
      const filename = playerSnapshot.current.split(/[\\/]/).pop() ?? "";
      setNowPlaying(filename, "");
    }
  }, [playerSnapshot, nowPlayingTrackId, setNowPlaying, refresh]);

  return (
    <footer className="flex items-center gap-3 border-t border-border bg-card px-4 py-2">
      <div className="flex min-w-0 flex-1 flex-col text-sm leading-tight">
        <span className="truncate font-semibold text-foreground">
          {nowPlayingTitle}
        </span>
        {nowPlayingArtist && (
          <span className="truncate text-xs text-muted-foreground">
            {nowPlayingArtist}
          </span>
        )}
      </div>
      {lyricsTrackId != null && (
        <>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setLyricsTrackId(lyricsTrackId)}
          >
            Lyrics
          </Button>
          <Separator orientation="vertical" className="h-6" />
        </>
      )}
      <div className="flex items-center gap-1">
        <Button variant="outline" size="icon" onClick={prev} title="previous">
          <SkipBack />
        </Button>
        <Button
          variant="default"
          size="icon"
          onClick={nowPlayingTrackId != null ? play : undefined}
          title="play"
        >
          <Play />
        </Button>
        <Button variant="outline" size="icon" onClick={pause} title="pause">
          <Pause />
        </Button>
        <Button variant="outline" size="icon" onClick={stop} title="stop">
          <Square />
        </Button>
        <Button variant="outline" size="icon" onClick={next} title="next">
          <SkipForward />
        </Button>
      </div>
      <LyricsDialog />
    </footer>
  );
}
