import { useRef } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { AlbumCover } from "@/components/album-cover";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useStore } from "@/lib/store";
import { addAlbumToQueue } from "@/lib/queue-actions";
import { useVisibleFit } from "@/hooks/use-visible-fit";

export function AlbumsList() {
  const items = useStore((s) => s.albumsList);
  const selectAlbum = useStore((s) => s.selectAlbum);
  const setAlbumsPageSize = useStore((s) => s.setAlbumsPageSize);

  const containerRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLTableRowElement>(null);

  const fit = useVisibleFit({
    containerRef,
    probeRef,
    minCellWidth: 36,
    columns: 1,
    gap: 0,
    headerRows: 1,
    onChange: setAlbumsPageSize,
  });

  return (
    <div ref={containerRef} className="flex-1 overflow-hidden">
      {items.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">No albums.</div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12"></TableHead>
              <TableHead>Album</TableHead>
              <TableHead>Artist</TableHead>
              <TableHead className="w-20">Tracks</TableHead>
              <TableHead className="w-16 text-right">Year</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.map((a, i) => (
              <ContextMenu key={a.id}>
                <ContextMenuTrigger asChild>
                  <TableRow
                    ref={i === 0 ? probeRef : null}
                    className="cursor-pointer"
                    onDoubleClick={() => selectAlbum(a.id)}
                  >
                    <TableCell className="w-12 p-1">
                      <AlbumCover
                        albumId={a.id}
                        className="h-8 w-8 rounded"
                      />
                    </TableCell>
                    <TableCell className="font-medium">{a.title}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {a.artist_name ?? ""}
                    </TableCell>
                    <TableCell className="w-20 text-muted-foreground">
                      {a.track_count}
                    </TableCell>
                    <TableCell className="w-16 text-right text-muted-foreground tabular-nums">
                      {a.year ?? ""}
                    </TableCell>
                  </TableRow>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem onClick={() => selectAlbum(a.id)}>
                    Open
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() => void addAlbumToQueue(a.id, a.title)}
                  >
                    Add album to queue
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenu>
            ))}
            {Array.from({
              length: Math.max(0, fit.pageSize - items.length - 1),
            }).map((_, i) => (
              <TableRow key={`pad-${i}`} aria-hidden>
                <TableCell colSpan={5} className="h-9" />
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
