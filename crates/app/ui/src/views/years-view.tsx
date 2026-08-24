import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Calendar } from "lucide-react";
import { PaginationBar } from "@/components/pagination-bar";
import { useYearsPage } from "@/hooks/use-pages";

export function YearsView() {
  useYearsPage();
  const items = useStore((s) => s.yearsList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);
  const page = useStore((s) => s.years.page);
  const setYearsPage = useStore((s) => s.setYearsPage);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <div className="flex-1 overflow-auto">
        {items.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No years.</div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3">
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
        )}
      </div>
      <PaginationBar
        page={page.page}
        pageSize={page.pageSize}
        total={page.total}
        onChange={setYearsPage}
      />
    </div>
  );
}
