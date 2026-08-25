import type { ReactNode } from "react";
import { LayoutGrid, List } from "lucide-react";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Badge } from "@/components/ui/badge";
import { useStore } from "@/lib/store";
import { AlbumTracksTable } from "@/views/album-tracks-table";
import { TracksIcons } from "@/views/tracks-icons";
import { PaginationBar } from "@/components/pagination-bar";
import { ModeToggle2 } from "@/views/view-toolbar";
import { useFilteredTracksPage } from "@/hooks/use-pages";

type Props = {
  /** Filter identifying the drill-down target. */
  filter: {
    genre?: string | null;
    year?: number | null;
    artistId?: number | null;
    albumId?: number | null;
  };
  /** Root breadcrumb label, e.g. "Genres". */
  rootLabel: string;
  /** Current breadcrumb label, e.g. the genre name. */
  currentLabel: string;
  /** Icon shown next to the title. */
  icon: ReactNode;
  /** Clears the selection, returning to the parent grid. */
  onBack: () => void;
};

/** Shared drill-down detail for genre / year / artist / album
 *  selections: breadcrumb, header, and the paged track table. */
export function FilteredTracksDetail({
  filter,
  rootLabel,
  currentLabel,
  icon,
  onBack,
}: Props) {
  useFilteredTracksPage({
    genre: filter.genre ?? null,
    year: filter.year ?? null,
    artistId: filter.artistId ?? null,
    albumId: filter.albumId ?? null,
  });
  const tracks = useStore((s) => s.tracksList);
  const total = useStore((s) => s.tracks.page.total);
  const page = useStore((s) => s.tracks.page.page);
  const pageSize = useStore((s) => s.tracks.page.pageSize);
  const setTracksPage = useStore((s) => s.setTracksPage);
  const mode = useStore((s) => s.tracks.mode);
  const setTracksMode = useStore((s) => s.setTracksMode);

  return (
    <div className="flex flex-1 flex-col gap-4">
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbLink asChild>
              <button type="button" onClick={onBack}>
                {rootLabel}
              </button>
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>{currentLabel}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
          {icon}
        </div>
        <div className="text-2xl font-bold">{currentLabel}</div>
        <Badge variant="secondary">{total} tracks</Badge>
        <div className="ml-auto">
          <ModeToggle2
            value={mode}
            onChange={setTracksMode}
            options={[
              { value: "icons", label: <LayoutGrid className="h-4 w-4" /> },
              { value: "list", label: <List className="h-4 w-4" /> },
            ]}
          />
        </div>
      </div>
      {tracks.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground">
          No tracks indexed.
        </div>
      ) : mode === "list" ? (
        <AlbumTracksTable tracks={tracks} />
      ) : (
        <TracksIcons />
      )}
      <PaginationBar
        page={page}
        pageSize={pageSize}
        total={total}
        onChange={setTracksPage}
      />
    </div>
  );
}
