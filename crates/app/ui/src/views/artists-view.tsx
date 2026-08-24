import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { Music } from "lucide-react";
import { PaginationBar } from "@/components/pagination-bar";
import { useArtistsPage } from "@/hooks/use-pages";

export function ArtistsView() {
  useArtistsPage();
  const items = useStore((s) => s.artistsList);
  const setView = useStore((s) => s.setView);
  const setTracksFilter = useStore((s) => s.setTracksFilter);
  const page = useStore((s) => s.artists.page);
  const setArtistsPage = useStore((s) => s.setArtistsPage);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <div className="flex-1 overflow-auto">
        {items.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground">No artists.</div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
            {items.map((a) => (
              <Card
                key={a.id}
                className="flex cursor-pointer items-center gap-3 p-4 transition-colors hover:border-primary"
                onClick={() => {
                  setTracksFilter({
                    genre: null,
                    year: null,
                    artistId: a.id,
                    albumId: null,
                  });
                  setView("tracks");
                }}
              >
                <Music className="h-8 w-8 shrink-0 text-muted-foreground" />
                <div className="min-w-0">
                  <div className="break-words font-semibold">{a.name}</div>
                  <div className="break-words text-xs text-muted-foreground">
                    {a.track_count} tracks
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
        onChange={setArtistsPage}
      />
    </div>
  );
}
