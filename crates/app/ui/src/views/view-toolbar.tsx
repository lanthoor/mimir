import { LayoutGrid, List } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useStore, type ViewKey } from "@/lib/store";

const TOOLBAR_FOR: Record<ViewKey, () => React.ReactNode> = {
  tracks: () => <TracksToolbar />,
  albums: () => <AlbumsToolbar />,
  folders: () => <FoldersToolbar />,
  artists: () => null,
  genres: () => null,
  years: () => null,
};

export function ViewToolbar({ view }: { view: ViewKey }) {
  const render = TOOLBAR_FOR[view];
  return render ? (
    <div className="flex items-center justify-end">{render()}</div>
  ) : null;
}

function TracksToolbar() {
  const mode = useStore((s) => s.tracks.mode);
  const setMode = useStore((s) => s.setTracksMode);
  return (
    <ModeToggle2
      value={mode}
      onChange={setMode}
      options={[
        { value: "icons", label: <LayoutGrid className="h-4 w-4" /> },
        { value: "list", label: <List className="h-4 w-4" /> },
      ]}
    />
  );
}

function AlbumsToolbar() {
  const mode = useStore((s) => s.albums.mode);
  const setMode = useStore((s) => s.setAlbumsMode);
  return (
    <ModeToggle2
      value={mode}
      onChange={setMode}
      options={[
        { value: "icons", label: <LayoutGrid className="h-4 w-4" /> },
        { value: "list", label: <List className="h-4 w-4" /> },
      ]}
    />
  );
}

function FoldersToolbar() {
  const mode = useStore((s) => s.folders.mode);
  const setMode = useStore((s) => s.setFoldersMode);
  return (
    <ModeToggle2
      value={mode}
      onChange={(m) => {
        setMode(m);
        // Reset cwd when switching modes — they're different views.
        useStore.getState().setCwd(null);
      }}
      options={[
        { value: "icons", label: "Icons" },
        { value: "tree", label: "Tree" },
      ]}
    />
  );
}

type ModeToggle2Props<T extends string> = {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: React.ReactNode }[];
};

function ModeToggle2<T extends string>({
  value,
  onChange,
  options,
}: ModeToggle2Props<T>) {
  return (
    <div className="inline-flex overflow-hidden rounded-md border border-border">
      {options.map((o) => (
        <Button
          key={o.value}
          variant={value === o.value ? "default" : "ghost"}
          size="sm"
          className="rounded-none"
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </Button>
      ))}
    </div>
  );
}
