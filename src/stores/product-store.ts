import { create } from "zustand";

export type ProductView = "home" | "library" | "details";

interface ProductState {
  activeView: ProductView;
  selectedGameId: string | null;
  returnView: ProductView;
  returnFocusId: string | null;
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
  setView: (activeView) => set({ activeView }),
  openDetails: (selectedGameId, returnView, returnFocusId) =>
    set({ activeView: "details", selectedGameId, returnView, returnFocusId }),
  closeDetails: () =>
    set((state) => ({
      activeView: state.returnView,
      returnFocusId: state.returnFocusId,
    })),
}));
