import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "";
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function basename(path: string): string {
  const segs = path.split(/[\\/]/).filter((s) => s.length > 0);
  return segs[segs.length - 1] || path;
}

/**
 * Cover art for an album, served by the Rust host's `mimircover` custom
 * protocol (crates/app/src/lib.rs). Loaded natively by the webview —
 * browser-cached, decoded off the main JS path, never base64'd through
 * IPC — so the Albums grid just points `<img src>` here.
 */
export function albumCoverUrl(albumId: number): string {
  return `mimircover://localhost/cover/${albumId}`;
}
