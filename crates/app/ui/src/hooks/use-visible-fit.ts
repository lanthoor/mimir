// Dynamic page-size computation: measure the actual visible area of a
// container and return a `pageSize` that fills it exactly
// (columns × rows for grids, single-column row count for tables).
//
// The hook calls `onChange` whenever the measured fit changes — pass it
// the store's `setXPageSize(...)`. The receiving store action must
// clamp `page` to the new maximum so a shrunk viewport doesn't leave
// the user stranded on a now-too-high page.

import { useEffect, useLayoutEffect, useRef } from "react";
import type { RefObject } from "react";

export type FitResult = {
  columns: number;
  rowsVisible: number;
  pageSize: number;
  rowHeight: number;
};

export function useVisibleFit(opts: {
  containerRef: RefObject<HTMLElement | null>;
  /** The grid element. When present, row height is derived from the
   *  grid's laid-out total height — this converges to an exact fit
   *  even when the first card is shorter than its row's tallest card
   *  (wrapped titles, cover art). */
  gridRef?: RefObject<HTMLElement | null>;
  /** Fallback probe: any rendered child to measure when the grid
   *  hasn't painted yet (tables use this exclusively). */
  probeRef?: RefObject<HTMLElement | null>;
  /** CSS `minmax(...)` lower bound for grid layouts. Used to derive
   *  the column count with the same formula the browser uses for
   *  `repeat(auto-fill, minmax(M, 1fr))`. */
  minCellWidth: number;
  /** For tables: force a single column. */
  columns?: number;
  /** Grid gap in px (matches `gap-3` = 12). Tables pass 0. */
  gap?: number;
  /** Rows taken by a fixed header (tables pass 1). */
  headerRows?: number;
  onChange: (pageSize: number) => void;
}): FitResult {
  const onChangeRef = useRef(opts.onChange);
  onChangeRef.current = opts.onChange;
  const containerRef = opts.containerRef;
  const gridRef = opts.gridRef;
  const probeRef = opts.probeRef;
  const gap = opts.gap ?? 12;
  const headerRows = opts.headerRows ?? 0;

  const lastPageSize = useRef<number>(0);
  const lastFit = useRef<FitResult>({
    columns: 0,
    rowsVisible: 0,
    pageSize: 0,
    rowHeight: 0,
  });

  const compute = (): FitResult => {
    const container = containerRef.current;
    if (!container) return lastFit.current;
    const cw = container.clientWidth;
    const ch = container.clientHeight;

    // Match the browser's auto-fill track count exactly:
    // floor((cw + gap) / (M + gap)) tracks.
    const columns =
      opts.columns ??
      Math.max(1, Math.floor((cw + gap) / (opts.minCellWidth + gap)));

    // Row height, in order of preference:
    //  1. derived from the laid-out grid's total height (grids only),
    //  2. measured from the probe child (tables, first paint),
    //  3. deterministic fallback (tables ~36 px, cards ~minCellWidth).
    let rowHeight = 0;
    const grid = gridRef?.current;
    if (grid && opts.columns == null && grid.children.length > 0) {
      const rows = Math.max(1, Math.ceil(grid.children.length / columns));
      rowHeight = (grid.scrollHeight + gap) / rows - gap;
    }
    if (!rowHeight) {
      rowHeight = probeRef?.current?.getBoundingClientRect().height ?? 0;
    }
    if (!rowHeight) {
      rowHeight = opts.columns != null ? 36 : opts.minCellWidth;
    }

    const rowsVisible = Math.max(
      1,
      Math.floor((ch + gap) / (rowHeight + gap)) - headerRows,
    );
    const pageSize = columns * rowsVisible;
    return { columns, rowsVisible, pageSize, rowHeight };
  };

  const publish = () => {
    const fit = compute();
    lastFit.current = fit;
    if (fit.pageSize > 0 && fit.pageSize !== lastPageSize.current) {
      lastPageSize.current = fit.pageSize;
      onChangeRef.current(fit.pageSize);
    }
  };

  // Re-measure after every render: catches the grid painting (first
  // fetch completing) and any content-driven size change.
  useLayoutEffect(() => {
    publish();
  });

  // Re-measure when the container, grid, or probe resizes (window
  // resize, maximize, cover art loading, filter chips appearing).
  useEffect(() => {
    const targets: HTMLElement[] = [];
    if (containerRef.current) targets.push(containerRef.current);
    if (gridRef?.current) targets.push(gridRef.current);
    if (probeRef?.current) targets.push(probeRef.current);
    if (targets.length === 0) return;
    const ro = new ResizeObserver(() => publish());
    for (const t of targets) ro.observe(t);
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef, gridRef, probeRef]);

  return lastFit.current;
}
