import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAddFolder } from "@/components/add-folder-context";
import { startFolderAdd } from "@/hooks/use-scan-events";
import { toast } from "sonner";

export function AddFolderDialog() {
  const { open, setOpen } = useAddFolder();
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setPath("");
      setBusy(false);
    }
  }, [open]);

  const pick = async () => {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Select a music folder",
      });
      if (typeof picked === "string" && picked.length > 0) {
        setPath(picked);
      }
    } catch (e) {
      console.error("folder picker failed:", e);
      toast.error(`Folder picker failed: ${describeError(e)}`);
    }
  };

  const submit = async () => {
    const trimmed = path.trim();
    if (!trimmed) return;
    setBusy(true);
    setOpen(false);
    try {
      await startFolderAdd(trimmed);
    } catch (e) {
      toast.error(`Add folder failed: ${describeError(e)}`);
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) setOpen(false);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add folder</DialogTitle>
          <DialogDescription>
            Watch a directory and scan it for audio files. The scan runs in
            the background — close this dialog and the progress bar will
            appear at the top.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2">
          <Label htmlFor="add-folder-path">Folder path</Label>
          <div className="flex gap-2">
            <Input
              id="add-folder-path"
              placeholder="/abs/path/to/music"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && path.trim() && !busy) {
                  e.preventDefault();
                  void submit();
                }
              }}
              autoFocus
            />
            <Button type="button" variant="outline" onClick={pick}>
              <FolderOpen />
              Browse
            </Button>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={!path.trim() || busy}>
            Add
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
