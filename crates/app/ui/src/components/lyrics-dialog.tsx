import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useStore } from "@/lib/store";
import * as ipc from "@/lib/ipc";
import type { LyricsPayload } from "@/lib/types";

export function LyricsDialog() {
  const trackId = useStore((s) => s.lyricsTrackId);
  const setTrackId = useStore((s) => s.setLyricsTrackId);
  const [lyrics, setLyrics] = useState<LyricsPayload | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (trackId == null) {
      setLyrics(null);
      return;
    }
    setLoading(true);
    ipc
      .libraryTrackLyrics(trackId)
      .then((l) => setLyrics(l))
      .finally(() => setLoading(false));
  }, [trackId]);

  return (
    <Dialog
      open={trackId != null}
      onOpenChange={(o) => {
        if (!o) setTrackId(null);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Lyrics</DialogTitle>
          <DialogDescription>
            {lyrics
              ? `${lyrics.source}${lyrics.language ? ` · ${lyrics.language}` : ""}`
              : "Loading…"}
          </DialogDescription>
        </DialogHeader>
        <div className="max-h-[60vh] overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted/30 p-4 text-sm">
          {loading
            ? "…"
            : lyrics
              ? lyrics.text
              : "No lyrics for this track."}
        </div>
      </DialogContent>
    </Dialog>
  );
}
