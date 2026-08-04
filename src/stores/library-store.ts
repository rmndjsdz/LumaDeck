import { create } from "zustand";

import type { GameStatus } from "../features/catalog/game-types";

export type LibraryStatus = "all" | GameStatus;
export type LibrarySort = "title" | "recent" | "time";

interface LibraryState {
  query: string;
  status: LibraryStatus;
  sort: LibrarySort;
  queryVersion: number;
  queryCommitted: boolean;
  setQuery: (query: string) => void;
  setStatus: (status: LibraryStatus) => void;
  setSort: (sort: LibrarySort) => void;
  reset: () => void;
}

export const LIBRARY_DEFAULTS = {
  query: "",
  status: "all" as LibraryStatus,
  sort: "title" as LibrarySort,
  queryVersion: 0,
  queryCommitted: false,
};

export const useLibraryStore = create<LibraryState>((set) => ({
  ...LIBRARY_DEFAULTS,
  setQuery: (query) =>
    set((state) =>
      state.query === query
        ? { queryCommitted: true }
        : {
            query,
            queryVersion: state.queryVersion + 1,
            queryCommitted: true,
          },
    ),
  setStatus: (status) => set({ status }),
  setSort: (sort) => set({ sort }),
  reset: () => set(LIBRARY_DEFAULTS),
}));
