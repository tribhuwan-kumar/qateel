import { writable } from "svelte/store";

const STORAGE_KEY = "historySort";

export type SortBy = "time" | "name" | "size";
export type Order = "asc" | "desc";

export type HistorySort = {
  sortBy: SortBy;
  order: Order;
};

export const defaultHistorySort: HistorySort = {
  sortBy: "time",
  order: "desc",
};

export function loadHistorySort(): HistorySort {
  if (typeof window === "undefined" || typeof localStorage === "undefined") {
    return defaultHistorySort;
  }

  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultHistorySort;

    const parsed = JSON.parse(raw);
    const validSortBy =
      parsed?.sortBy === "time" ||
      parsed?.sortBy === "name" ||
      parsed?.sortBy === "size";
    const validOrder = parsed?.order === "asc" || parsed?.order === "desc";

    if (validSortBy && validOrder) {
      return {
        sortBy: parsed.sortBy,
        order: parsed.order,
      };
    }
  } catch {
    // ignore malformed localStorage data
  }

  return defaultHistorySort;
}

export const historySort = writable<HistorySort>(loadHistorySort());

historySort.subscribe((value) => {
  if (typeof window !== "undefined" && "localStorage" in window) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
  }
});

