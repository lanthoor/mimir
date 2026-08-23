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

export function AlbumsList() {
  const items = useStore((s) => s.albumsList);
  const selectAlbum = useStore((s) => s.selectAlbum);

  if (items.length === 0) {
    return (
      <div className="p-6 text-sm text-muted-foreground">No albums.</div>
    );
  }

  return (
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
        {items.map((a) => (
          <ContextMenu key={a.id}>
            <ContextMenuTrigger asChild>
              <TableRow
                className="cursor-pointer"
                onDoubleClick={() => selectAlbum(a.id)}
              >
                <TableCell className="w-12 p-1">
                  <AlbumCover albumId={a.id} className="h-8 w-8 rounded" />
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
      </TableBody>
    </Table>
  );
}
