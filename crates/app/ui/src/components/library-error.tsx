import { useStore } from "@/lib/store";

export function LibraryError() {
  const lastError = useStore((s) => s.library.last_error);
  if (!lastError) return null;
  return (
    <div className="m-4 rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive-foreground">
      <div className="font-semibold">Library could not be opened</div>
      <code className="mt-1 block break-words text-xs text-destructive">
        {lastError}
      </code>
    </div>
  );
}
