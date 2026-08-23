import { useMemo, useState } from "react";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Card } from "@/components/ui/card";
import { useStore } from "@/lib/store";
import { basename } from "@/lib/utils";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import * as ipc from "@/lib/ipc";
import { Music, ArrowUp, Folder } from "lucide-react";
import { toast } from "sonner";
import type { FolderFile, FolderNode } from "@/lib/types";
import { EditTrackDialog } from "@/components/edit-track-dialog";

export function FoldersIcons() {
  const tree = useStore((s) => s.folderTree);
  const cwd = useStore((s) => s.folders.cwd);
  const setCwd = useStore((s) => s.setCwd);
  const setNowPlaying = useStore((s) => s.setNowPlaying);
  const setNowPlayingTrackId = useStore((s) => s.setNowPlayingTrackId);
  const setLyricsTrackId = useStore((s) => s.setLyricsTrackId);
  const setFolderTree = useStore((s) => s.setFolderTree);

  const current = useMemo(() => {
    if (cwd == null) return null;
    return findByPath(tree.root_children, cwd);
  }, [tree, cwd]);

  const nodes = current?.children ?? tree.root_children;
  const files = current?.files ?? [];

  if (tree.root_children.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
        No folders with tracks yet. Click + folder to watch a directory.
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <FolderBreadcrumb
        current={current}
        tree={tree.root_children}
        onJump={setCwd}
      />
      <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-3">
        {current != null && (
          <Card
            className="flex cursor-pointer flex-col items-center gap-1 border-dashed p-3 text-muted-foreground transition-colors hover:border-primary"
            onDoubleClick={() => {
              const parent = current.path.replace(/\/[^/]+$/, "") || null;
              setCwd(parent === current.path ? null : parent);
            }}
          >
            <ArrowUp className="h-8 w-8" />
            <div className="text-sm font-semibold">..</div>
            <div className="text-xs">Up</div>
          </Card>
        )}
        {nodes.map((n) => (
          <FolderIcon
            key={n.path}
            node={n}
            onOpen={() => setCwd(n.path)}
            onRemove={
              n.folder_id != null
                ? () => {
                    const folderId = n.folder_id!;
                    if (!confirm(`Stop watching this folder?\n\n${n.path}`)) return;
                    ipc
                      .libraryRemoveFolder(folderId)
                      .then(() => {
                        toast.success(`Removed folder ${n.path}`);
                        return ipc.libraryFolderTree();
                      })
                      .then((t) => {
                        setFolderTree(t);
                        setCwd(null);
                      })
                      .catch((e) =>
                        toast.error(`Remove failed: ${describeError(e)}`),
                      );
                  }
                : undefined
            }
          />
        ))}
        {files.map((f) => (
          <FileIcon
            key={f.path}
            file={f}
            onPlay={() => {
              if (f.track_id == null) return;
              setNowPlaying(f.title ?? basename(f.path), "");
              setNowPlayingTrackId(f.track_id);
              setLyricsTrackId(f.track_id);
              ipc.audioPlay(f.track_id).catch(console.error);
            }}
          />
        ))}
        {nodes.length === 0 && files.length === 0 && (
          <div className="col-span-full rounded-md border border-dashed border-border p-4 text-center text-sm text-muted-foreground">
            This folder is empty.
          </div>
        )}
      </div>
    </div>
  );
}

function FolderIcon({
  node,
  onOpen,
  onRemove,
}: {
  node: FolderNode;
  onOpen: () => void;
  onRemove?: () => void;
}) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <Card
          className="flex cursor-pointer flex-col items-center gap-1 p-3 transition-colors hover:border-primary"
          onDoubleClick={onOpen}
        >
          <Folder className="h-10 w-10 text-muted-foreground" />
          <div className="truncate text-center text-sm font-semibold">
            {node.name ?? basename(node.path)}
          </div>
          <div className="truncate text-center text-xs text-muted-foreground">
            {node.path}
          </div>
        </Card>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={onOpen}>Open</ContextMenuItem>
        {onRemove && (
          <ContextMenuItem
            onClick={onRemove}
            className="text-destructive focus:text-destructive"
          >
            Remove
          </ContextMenuItem>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}

function FileIcon({
  file,
  onPlay,
}: {
  file: FolderFile;
  onPlay: () => void;
}) {
  const [editing, setEditing] = useState(false);
  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <Card
            className="flex cursor-pointer flex-col items-center gap-1 p-3 transition-colors hover:border-primary"
            onDoubleClick={onPlay}
            title={file.path}
          >
            <Music className="h-10 w-10 text-muted-foreground" />
            <div className="truncate text-center text-sm font-semibold">
              {file.title ?? basename(file.path)}
            </div>
            <div className="truncate text-center text-xs text-muted-foreground">
              {basename(file.path)}
            </div>
          </Card>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem
            disabled={file.track_id == null}
            onClick={onPlay}
          >
            Play
          </ContextMenuItem>
          <ContextMenuItem
            disabled={file.track_id == null}
            onClick={() => setEditing(true)}
          >
            Edit…
          </ContextMenuItem>
          <ContextMenuItem
            onClick={() =>
              ipc.libraryRevealInFileManager(file.path).catch(console.error)
            }
          >
            Reveal in file manager
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {editing && file.track_id != null && (
        <EditTrackDialog
          trackId={file.track_id}
          open={editing}
          onOpenChange={setEditing}
        />
      )}
    </>
  );
}

function FolderBreadcrumb({
  current,
  tree,
  onJump,
}: {
  current: FolderNode | null;
  tree: FolderNode[];
  onJump: (path: string | null) => void;
}) {
  const trail = useMemo(() => {
    if (current == null) return [];
    const all = collectAllNodes(tree);
    const out: FolderNode[] = [];
    let cur: FolderNode | null = current;
    while (cur) {
      out.unshift(cur);
      cur =
        all.find((n) => (n.children || []).some((c) => c.path === cur!.path)) ??
        null;
    }
    return out;
  }, [current, tree]);

  if (trail.length === 0) return null;
  return (
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink asChild>
            <button type="button" onClick={() => onJump(null)}>
              Folders
            </button>
          </BreadcrumbLink>
        </BreadcrumbItem>
        {trail.map((node, i) => {
          const isLast = i === trail.length - 1;
          const label = node.name ?? basename(node.path);
          return (
            <span key={node.path} className="contents">
              <BreadcrumbSeparator />
              <BreadcrumbItem>
                {isLast ? (
                  <BreadcrumbPage>{label}</BreadcrumbPage>
                ) : (
                  <BreadcrumbLink asChild>
                    <button type="button" onClick={() => onJump(node.path)}>
                      {label}
                    </button>
                  </BreadcrumbLink>
                )}
              </BreadcrumbItem>
            </span>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}

function findByPath(
  nodes: FolderNode[],
  path: string,
): FolderNode | null {
  for (const n of nodes) {
    if (n.path === path) return n;
    const inChild = findByPath(n.children, path);
    if (inChild) return inChild;
  }
  return null;
}

function collectAllNodes(nodes: FolderNode[]): FolderNode[] {
  const out: FolderNode[] = [];
  const stack = [...nodes];
  while (stack.length) {
    const n = stack.pop()!;
    out.push(n);
    if (n.children) stack.push(...n.children);
  }
  return out;
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
