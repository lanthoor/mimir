import { useMemo } from "react";
import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationFirst,
  PaginationItem,
  PaginationLast,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from "@/components/ui/pagination";

type PaginationBarProps = {
  page: number;
  pageSize: number;
  total: number;
  onChange: (page: number) => void;
  className?: string;
};

const FIRST = 1;

export function PaginationBar({
  page,
  pageSize,
  total,
  onChange,
  className,
}: PaginationBarProps) {
  const lastPage = Math.max(FIRST, Math.ceil(total / pageSize));
  const items = useMemo(
    () => buildPageItems(page, lastPage),
    [page, lastPage],
  );

  if (total <= pageSize) {
    return null;
  }

  const goto = (n: number) => () => {
    if (n < FIRST || n > lastPage || n === page) return;
    onChange(n);
  };

  return (
    <Pagination className={className}>
      <PaginationContent>
        <PaginationItem>
          <PaginationFirst
            onClick={goto(FIRST)}
            aria-disabled={page === FIRST}
            tabIndex={page === FIRST ? -1 : 0}
            className={
              page === FIRST ? "pointer-events-none opacity-50" : "cursor-pointer"
            }
          />
        </PaginationItem>
        <PaginationItem>
          <PaginationPrevious
            onClick={goto(page - 1)}
            aria-disabled={page === FIRST}
            tabIndex={page === FIRST ? -1 : 0}
            className={
              page === FIRST ? "pointer-events-none opacity-50" : "cursor-pointer"
            }
          />
        </PaginationItem>

        {items.map((it, i) =>
          it === "ellipsis" ? (
            <PaginationItem key={`e-${i}`}>
              <PaginationEllipsis />
            </PaginationItem>
          ) : (
            <PaginationItem key={it}>
              <PaginationLink
                isActive={it === page}
                onClick={goto(it)}
                className="cursor-pointer"
              >
                {it}
              </PaginationLink>
            </PaginationItem>
          ),
        )}

        <PaginationItem>
          <PaginationNext
            onClick={goto(page + 1)}
            aria-disabled={page === lastPage}
            tabIndex={page === lastPage ? -1 : 0}
            className={
              page === lastPage
                ? "pointer-events-none opacity-50"
                : "cursor-pointer"
            }
          />
        </PaginationItem>
        <PaginationItem>
          <PaginationLast
            onClick={goto(lastPage)}
            aria-disabled={page === lastPage}
            tabIndex={page === lastPage ? -1 : 0}
            className={
              page === lastPage
                ? "pointer-events-none opacity-50"
                : "cursor-pointer"
            }
          />
        </PaginationItem>
      </PaginationContent>
    </Pagination>
  );
}

/// Build the list of numbered buttons + ellipses shown between prev/next.
/// 1 and lastPage are always rendered so the user always sees the
/// boundaries — including when sitting on one of them.
function buildPageItems(page: number, lastPage: number): (number | "ellipsis")[] {
  if (lastPage <= 1) return [];
  if (lastPage <= 7) {
    return Array.from({ length: lastPage }, (_, i) => i + 1);
  }

  const items: (number | "ellipsis")[] = [1];

  // Pages around `page`, exclusive of 1 and lastPage (which we render
  // independently so they never disappear).
  const start = Math.max(2, page - 1);
  const end = Math.min(lastPage - 1, page + 1);

  if (start > 2) items.push("ellipsis");
  for (let p = start; p <= end; p++) items.push(p);
  if (end < lastPage - 1) items.push("ellipsis");
  items.push(lastPage);

  return items;
}
