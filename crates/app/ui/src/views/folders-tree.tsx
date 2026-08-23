import { useState } from "react";
import { ChevronDown, ChevronRight, Music, Folder } from "lucide-react";
import { useStore } from "@/lib/store";
import { basename } from "@/lib/utils";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import * as ipc from "@/lib/ipc";
import { toast } from "sonner";
import type { FolderNode } from "@/lib/types";

export function FoldersTree() {
  const tree = useStore((s) => s.folderTree);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);
  const setFolderTree = useStore((s) => s.setFolderTree);

  if (tree.root_children.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
        No folders with tracks yet.
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1">
      {tree.root_children.map((root) => (
        <TreeNode
          key={root.path}
          node={root}
          depth={0}
          onPlay={(file) => {
            if (file.track_id == null) return;
            setNowPlaying(file.title ?? basename(file.path), "");
            setNowPlayingTrackId(file.track_id);
            setLyricsTrackId(file.track_id);
            ipc.audioPlay(file.track_id).catch(console.error);
          }}
          onRemoveFolder={(id, path) => {
            if (!confirm(`Stop watching this folder?\n\n${path}`)) return;
            ipc
              .libraryRemoveFolder(id)
              .then(() => ipc.libraryFolderTree())
              .then((t) => setFolderTree(t))
              .catch((e) =>
                toast.error(`Remove failed: ${describeError(e)}`),
              );
          }}
        />
      ))}
    </div>
  );
}

type Props = {
  node: FolderNode;
  depth: number;
  onPlay: (file: import("@/lib/types").FolderFile) => void;
  onRemoveFolder: (id: number, path: string) => void;
};

function TreeNode({ node, depth, onPlay, onRemoveFolder }: Props) {
  const [open, setOpen] = useState(depth < 2);
  const hasChildren =
    (node.children && node.children.length > 0) || node.files.length > 0;
  const paddingLeft = depth * 16;

  return (
    <div>
      <div
        className="flex items-center gap-1 rounded px-1 py-0.5 hover:bg-accent/40"
        style={{ paddingLeft }}
      >
        {hasChildren ? (
          <button
            type="button"
            className="flex h-5 w-5 items-center justify-center text-muted-foreground"
            onClick={() => setOpen((o) => !o)}
          >
            {open ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
          </button>
        ) : (
          <span className="inline-block h-5 w-5" />
        )}
        <ContextMenu>
          <ContextMenuTrigger asChild>
            <span
              className="flex flex-1 cursor-default items-center gap-2 truncate text-sm"
              onDoubleClick={() => {
                if (node.folder_id != null) setOpen((o) => !o);
              }}
            >
              <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
              <span className="truncate font-semibold">
                {node.name ?? basename(node.path)}
              </span>
              <span className="ml-2 text-xs text-muted-foreground">
                {node.files.length} files
              </span>
            </span>
          </ContextMenuTrigger>
          {node.folder_id != null && (
            <ContextMenuContent>
              <ContextMenuItem
                onClick={() => onRemoveFolder(node.folder_id!, node.path)}
                className="text-destructive focus:text-destructive"
              >
                Remove
              </ContextMenuItem>
            </ContextMenuContent>
          )}
        </ContextMenu>
      </div>
      {open && (
        <div className="flex flex-col gap-0.5">
          {node.files.map((f) => (
            <FileTreeNode
              key={f.path}
              file={f}
              depth={depth + 1}
              onPlay={onPlay}
            />
          ))}
          {node.children.map((c) => (
            <TreeNode
              key={c.path}
              node={c}
              depth={depth + 1}
              onPlay={onPlay}
              onRemoveFolder={onRemoveFolder}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function FileTreeNode({
  file,
  depth,
  onPlay,
}: {
  file: import("@/lib/types").FolderFile;
  depth: number;
  onPlay: (file: import("@/lib/types").FolderFile) => void;
}) {
  return (
    <div
      className="flex cursor-pointer items-center gap-2 truncate rounded px-1 py-0.5 text-sm text-muted-foreground hover:bg-accent/40"
      style={{ paddingLeft: depth * 16 }}
      onDoubleClick={() => onPlay(file)}
      title={file.path}
    >
      <Music className="h-3 w-3 shrink-0" />
      <span className="truncate">
        {file.title ?? basename(file.path)}
      </span>
    </div>
  );
}

function describeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}
