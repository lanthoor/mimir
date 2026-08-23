import { useStore } from "@/lib/store";

export function ScanProgress() {
  const scanning = useStore((s) => s.scanning);
  const active = scanning > 0;
  return (
    <div
      className="relative h-[3px] w-full overflow-hidden bg-transparent"
      hidden={!active}
      aria-hidden={!active}
    >
      {active && <div className="h-full w-full animate-indeterminate bg-primary" />}
    </div>
  );
}
