import { useMemo } from "react";
import { useStore } from "@/lib/store";
import { TracksIcons } from "@/views/tracks-icons";
import { TracksList } from "@/views/tracks-list";
import { ViewToolbar } from "@/views/view-toolbar";

export function TracksView() {
  const mode = useStore((s) => s.tracks.mode);
  const filterChips = useMemo(() => {
    const f = useStore.getState().tracks.filter;
    return [
      f.genre ? { key: "genre", label: `genre=${f.genre}` } : null,
      f.year != null ? { key: "year", label: `year=${f.year}` } : null,
      f.artistId != null ? { key: "artist", label: `artist=${f.artistId}` } : null,
      f.albumId != null ? { key: "album", label: `album=${f.albumId}` } : null,
    ].filter((c): c is { key: string; label: string } => c !== null);
  }, [useStore((s) => s.tracks.filter)]);

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <ViewToolbar view="tracks" />
      {filterChips.length > 0 && <FilterChips chips={filterChips} />}
      <div className="flex-1 overflow-auto">
        {mode === "list" ? <TracksList /> : <TracksIcons />}
      </div>
    </div>
  );
}

function FilterChips({ chips }: { chips: { key: string; label: string }[] }) {
  const setTracksFilter = useStore((s) => s.setTracksFilter);
  return (
    <div className="flex flex-wrap gap-1">
      {chips.map((c) => (
        <button
          key={c.key}
          type="button"
          onClick={() =>
            setTracksFilter({
              genre: null,
              year: null,
              artistId: null,
              albumId: null,
            })
          }
          className="rounded-full border border-primary px-3 py-0.5 text-xs hover:bg-primary hover:text-primary-foreground"
        >
          {c.label} ✕
        </button>
      ))}
    </div>
  );
}
