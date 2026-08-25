import { useRef } from "react";
import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Calendar } from "lucide-react";
import { PaginationBar } from "@/components/pagination-bar";
import { useYearsPage } from "@/hooks/use-pages";
import { useVisibleFit } from "@/hooks/use-visible-fit";
import { FilteredTracksDetail } from "@/views/filtered-tracks-detail";

export function YearsView() {
  useYearsPage();
  const items = useStore((s) => s.yearsList);
  const selectedYear = useStore((s) => s.years.selectedYear);
  const selectYear = useStore((s) => s.selectYear);
  const page = useStore((s) => s.years.page);
  const setYearsPage = useStore((s) => s.setYearsPage);
  const setYearsPageSize = useStore((s) => s.setYearsPageSize);

  const containerRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);

  const fit = useVisibleFit({
    containerRef,
    gridRef,
    probeRef,
    minCellWidth: 160,
    gap: 12,
    onChange: setYearsPageSize,
  });

  if (selectedYear != null) {
    return (
      <div className="flex h-full flex-col gap-2 p-4">
        <FilteredTracksDetail
          filter={{ year: selectedYear }}
          rootLabel="Years"
          currentLabel={String(selectedYear)}
          icon={<Calendar className="h-5 w-5" />}
          onBack={() => selectYear(null)}
        />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <div ref={containerRef} className="flex-1 overflow-hidden">
        {items.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No years.</div>
        ) : (
          <div
            ref={gridRef}
            className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3"
          >
            {items.map((y, i) => (
              <Card
                key={y.year}
                ref={i === 0 ? probeRef : null}
                className="flex cursor-pointer items-center gap-3 p-4 transition-colors hover:border-primary"
                onClick={() => selectYear(y.year)}
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
        onChange={setYearsPage}
      />
    </div>
  );
}
