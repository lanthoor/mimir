import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Tags } from "lucide-react";

export function GenresView() {
  const items = useStore((s) => s.genresList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);

  if (items.length === 0) {
    return (
      <div className="p-6 text-sm text-muted-foreground">No genres.</div>
    );
  }

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3 p-4">
      {items.map((g) => (
        <Card
          key={g.name}
          className="flex cursor-pointer items-center gap-3 p-4 transition-colors hover:border-primary"
          onClick={() => {
            setTracksFilter({
              genre: g.name,
              year: null,
              artistId: null,
              albumId: null,
            });
            setView("tracks");
          }}
        >
          <Tags className="h-8 w-8 shrink-0 text-muted-foreground" />
          <div className="min-w-0">
            <div className="truncate font-semibold">{g.name}</div>
            <div className="truncate text-xs text-muted-foreground">
              {g.track_count} tracks
            </div>
          </div>
        </Card>
      ))}
    </div>
  );
}
