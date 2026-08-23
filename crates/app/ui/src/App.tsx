import { useEffect } from "react";
import { Toaster } from "sonner";
import { Nav } from "@/components/nav";
import { ScanProgress } from "@/components/scan-progress";
import { NowPlaying } from "@/components/now-playing";
import { AddFolderDialog } from "@/components/add-folder-dialog";
import { AddFolderProvider } from "@/components/add-folder-context";
import { LibraryError } from "@/components/library-error";
import { ThemeProvider } from "@/components/theme-provider";
import { useLibrary } from "@/hooks/use-library";
import { useScanEvents } from "@/hooks/use-scan-events";
import { useStore } from "@/lib/store";
import { TracksView } from "@/views/tracks-view";
import { AlbumsView } from "@/views/albums-view";
import { ArtistsView } from "@/views/artists-view";
import { GenresView } from "@/views/genres-view";
import { YearsView } from "@/views/years-view";
import { FoldersView } from "@/views/folders-view";
import { QueueView } from "@/views/queue-view";

function ViewRouter() {
  const view = useStore((s) => s.view);
  switch (view) {
    case "tracks":
      return <TracksView />;
    case "albums":
      return <AlbumsView />;
    case "genres":
      return <GenresView />;
    case "years":
      return <YearsView />;
    case "folders":
      return <FoldersView />;
    case "queue":
      return <QueueView />;
    case "artists":
      return <ArtistsView />;
  }
}

function LibraryBootstrap() {
  const { refresh, refreshStatus } = useLibrary();
  const view = useStore((s) => s.view);
  useScanEvents();

  useEffect(() => {
    refreshStatus().catch(console.error);
  }, [refreshStatus]);

  useEffect(() => {
    refresh().catch(console.error);
  }, [refresh, view]);

  return null;
}

export function App() {
  return (
    <ThemeProvider defaultTheme="system">
      <AddFolderProvider>
        <LibraryBootstrap />
        <div className="flex h-full flex-col bg-background text-foreground">
          <Nav />
          <ScanProgress />
          <main className="flex-1 overflow-auto">
            <LibraryError />
            <ViewRouter />
          </main>
          <NowPlaying />
        </div>
        <AddFolderDialog />
        <Toaster position="top-right" richColors closeButton />
      </AddFolderProvider>
    </ThemeProvider>
  );
}
