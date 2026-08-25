import { useRef } from "react";
import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Tags } from "lucide-react";
import { PaginationBar } from "@/components/pagination-bar";
import { useGenresPage } from "@/hooks/use-pages";
import { useVisibleFit } from "@/hooks/use-visible-fit";

export function GenresView() {
  useGenresPage();
  const items = useStore((s) => s.genresList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);
  const page = useStore((s) => s.genres.page);
  const setGenresPage = useStore((s) => s.setGenresPage);
  const setGenresPageSize = useStore((s) => s.setGenresPageSize);

  const containerRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);

  const fit = useVisibleFit({
    containerRef,
    gridRef,
    probeRef,
    minCellWidth: 180,
    gap: 12,
    onChange: setGenresPageSize,
  });

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <div ref={containerRef} className="flex-1 overflow-hidden">
        {items.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No genres.</div>
        ) : (
          <div
            ref={gridRef}
            className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3"
          >
            {items.map((g, i) => (
              <Card
                key={g.name}
                ref={i === 0 ? probeRef : null}
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
                  <div className="break-words font-semibold">{g.name}</div>
                  <div className="break-words text-xs text-muted-foreground">
                    {g.track_count} tracks
                  </div>
                </div>
              </Card>
            ))}
            {Array.from({
              length: Math.max(0, fit.pageSize - items.length),
            }).map((_, i) => (
              <div
                key={`pad-${i}`}
                aria-hidden
                className="invisible rounded-md border border-transparent"
                style={{ height: fit.rowHeight || undefined }}
                data-placeholder
              />
            ))}
          </div>
        )}
      </div>
      <PaginationBar
        page={page.page}
        pageSize={page.pageSize}
        total={page.total}
        onChange={setGenresPage}
      />
    </div>
  );
}
