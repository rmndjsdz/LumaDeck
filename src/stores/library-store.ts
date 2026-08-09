import { create } from "zustand";

import type { GameStatus } from "../features/catalog/game-types";
import type { LibraryGenreId } from "../features/launcher/library-operations";

export type LibraryStatus = "all" | GameStatus;
export type LibrarySort = "title" | "recent" | "time";

interface LibraryState {
  query: string;
  status: LibraryStatus;
  sort: LibrarySort;
  genre: LibraryGenreId;
  queryVersion: number;
  queryCommitted: boolean;
  setQuery: (query: string) => void;
  setStatus: (status: LibraryStatus) => void;
  setSort: (sort: LibrarySort) => void;
  setGenre: (genre: LibraryGenreId) => void;
  reset: () => void;
}

export const LIBRARY_DEFAULTS = {
  query: "",
  status: "all" as LibraryStatus,
  sort: "title" as LibrarySort,
  genre: "all" as LibraryGenreId,
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
  setGenre: (genre) => set({ genre }),
  reset: () => set(LIBRARY_DEFAULTS),
}));
