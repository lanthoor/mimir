import { useState } from "react";
import { Disc3 } from "lucide-react";
import { albumCoverUrl, cn } from "@/lib/utils";

type Props = { albumId: number; className?: string };

/**
 * Album cover art with a placeholder fallback. The image comes straight
 * from the host's `mimircover` protocol (see `albumCoverUrl`), so the
 * webview handles its own caching — nothing is routed through JS/IPC.
 */
export function AlbumCover({ albumId, className }: Props) {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return (
      <div
        className={cn(
          "flex items-center justify-center bg-muted text-muted-foreground",
          className,
        )}
      >
        <Disc3 className="h-1/3 w-1/3" />
      </div>
    );
  }
  return (
    <div className={cn("overflow-hidden bg-muted", className)}>
      <img
        src={albumCoverUrl(albumId)}
        alt=""
        loading="lazy"
        draggable={false}
        className="h-full w-full object-cover"
        onError={() => setFailed(true)}
      />
    </div>
  );
}
