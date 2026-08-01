import { createContext, useContext } from "react";

import type {
  GridNavigationConfig,
  LinearNavigationConfig,
} from "../core/navigation-types";

export interface RowNavigationContextValue {
  groupId: string;
  rowId: string;
  rowIndex: number;
  preserveHorizontalIntent: boolean;
}

export const LinearNavigationContext =
  createContext<LinearNavigationConfig | null>(null);

export function useLinearNavigation(): LinearNavigationConfig | null {
  return useContext(LinearNavigationContext);
}

export const RowNavigationContext =
  createContext<RowNavigationContextValue | null>(null);

export function useRowNavigation(): RowNavigationContextValue | null {
  return useContext(RowNavigationContext);
}

export const GridNavigationContext = createContext<GridNavigationConfig | null>(
  null,
);

export function useGridNavigation(): GridNavigationConfig | null {
  return useContext(GridNavigationContext);
}
