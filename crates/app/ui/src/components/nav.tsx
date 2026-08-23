import { Search, FolderPlus } from "lucide-react";
import { useStore, type ViewKey } from "@/lib/store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { ModeToggle } from "@/components/mode-toggle";
import { useAddFolder } from "@/components/add-folder-context";

const VIEWS: { key: ViewKey; label: string }[] = [
  { key: "tracks", label: "Tracks" },
  { key: "albums", label: "Albums" },
  { key: "artists", label: "Artists" },
  { key: "genres", label: "Genres" },
  { key: "years", label: "Years" },
  { key: "folders", label: "Folders" },
];

export function Nav() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const tracksQuery = useStore((s) => s.tracks.query);
  const setTracksQuery = useStore((s) => s.setTracksQuery);
  const libraryError = useStore((s) => s.library.last_error);
  const { setOpen: openAddFolder } = useAddFolder();

  return (
    <header className="flex items-center gap-2 border-b border-border bg-card px-4 py-2">
      <h1 className="mr-2 text-lg font-semibold uppercase tracking-widest text-foreground">
        mimir
      </h1>
      <nav className="flex gap-1">
        {VIEWS.map((v) => (
          <Button
            key={v.key}
            variant={view === v.key ? "default" : "ghost"}
            size="sm"
            onClick={() => setView(v.key)}
            className={cn(
              "text-sm",
              view === v.key && "bg-primary text-primary-foreground",
            )}
          >
            {v.label}
          </Button>
        ))}
      </nav>
      <div className="ml-auto flex items-center gap-2">
        {view === "tracks" && (
          <div className="relative">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              type="search"
              placeholder="search…"
              className="w-64 pl-8"
              value={tracksQuery}
              onChange={(e) => setTracksQuery(e.target.value)}
            />
          </div>
        )}
        <Button
          variant="default"
          size="sm"
          onClick={() => openAddFolder(true)}
          disabled={!!libraryError}
          title={libraryError ? "Library isn't open" : "Add a folder"}
        >
          <FolderPlus />
          folder
        </Button>
        <ModeToggle />
      </div>
    </header>
  );
}
