import { useStore } from "@/lib/store";
import { FoldersIcons } from "@/views/folders-icons";
import { FoldersTree } from "@/views/folders-tree";
import { ViewToolbar } from "@/views/view-toolbar";

export function FoldersView() {
  const mode = useStore((s) => s.folders.mode);
  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <ViewToolbar view="folders" />
      <div className="flex-1 overflow-auto">
        {mode === "tree" ? <FoldersTree /> : <FoldersIcons />}
      </div>
    </div>
  );
}
