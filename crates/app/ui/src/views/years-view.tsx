import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Calendar } from "lucide-react";

export function YearsView() {
  const items = useStore((s) => s.yearsList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);

  if (items.length === 0) {
    return (
      <div className="p-6 text-sm text-muted-foreground">No years.</div>
    );
  }

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3 p-4">
      {items.map((y) => (
        <Card
          key={y.year}
          className="flex cursor-pointer items-center gap-3 p-4 transition-colors hover:border-primary"
          onClick={() => {
            setTracksFilter({
              genre: null,
              year: y.year,
              artistId: null,
              albumId: null,
            });
            setView("tracks");
          }}
        >
          <Calendar className="h-8 w-8 shrink-0 text-muted-foreground" />
          <div>
            <div className="text-lg font-semibold tabular-nums">
              {y.year}
            </div>
            <div className="text-xs text-muted-foreground">
              {y.track_count} tracks
            </div>
          </div>
        </Card>
      ))}
    </div>
  );
}
