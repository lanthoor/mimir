import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Tags } from "lucide-react";
import { PaginationBar } from "@/components/pagination-bar";
import { useGenresPage } from "@/hooks/use-pages";

export function GenresView() {
  useGenresPage();
  const items = useStore((s) => s.genresList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);
  const page = useStore((s) => s.genres.page);
  const setGenresPage = useStore((s) => s.setGenresPage);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <div className="flex-1 overflow-auto">
        {items.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No genres.</div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
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
                  <div className="break-words font-semibold">{g.name}</div>
                  <div className="break-words text-xs text-muted-foreground">
                    {g.track_count} tracks
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
        onChange={setGenresPage}
      />
    </div>
  );
}
