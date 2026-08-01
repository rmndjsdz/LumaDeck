import { create } from "zustand";

export type ProductView = "home" | "library" | "details";

interface ProductState {
  activeView: ProductView;
  selectedGameId: string | null;
  returnView: ProductView;
  returnFocusId: string | null;
  viewTransitionId: number;
  setView: (view: ProductView) => void;
  openDetails: (
    gameId: string,
    returnView: ProductView,
    openerFocusId: string | null,
  ) => void;
  closeDetails: () => void;
}

export const useProductStore = create<ProductState>((set) => ({
  activeView: "home",
  selectedGameId: null,
  returnView: "home",
  returnFocusId: null,
  viewTransitionId: 0,
  setView: (activeView) =>
    set((state) => ({
      activeView,
      viewTransitionId: state.viewTransitionId + 1,
    })),
  openDetails: (selectedGameId, returnView, returnFocusId) =>
    set((state) => ({
      activeView: "details",
      selectedGameId,
      returnView,
      returnFocusId,
      viewTransitionId: state.viewTransitionId + 1,
    })),
  closeDetails: () =>
    set((state) => ({
      activeView: state.returnView,
      returnFocusId: state.returnFocusId,
      viewTransitionId: state.viewTransitionId + 1,
    })),
}));
