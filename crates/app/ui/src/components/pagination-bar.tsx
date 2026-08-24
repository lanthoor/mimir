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

function buildPageItems(page: number, lastPage: number): (number | "ellipsis")[] {
  if (lastPage <= 7) {
    return Array.from({ length: lastPage }, (_, i) => i + 1);
  }
  const items: (number | "ellipsis")[] = [];
  const window: number[] = [page - 1, page, page + 1].filter(
    (p) => p > 1 && p < lastPage,
  );
  if (page > 3) {
    items.push(1, "ellipsis");
  } else {
    for (let p = 1; p < page; p++) items.push(p);
  }
  for (const p of window) items.push(p);
  if (page < lastPage - 2) {
    items.push("ellipsis", lastPage);
  } else {
    for (let p = page + 1; p <= lastPage; p++) items.push(p);
  }
  return items;
}
