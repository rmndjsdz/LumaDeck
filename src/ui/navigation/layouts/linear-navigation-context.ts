import { createContext, useContext } from "react";

import type {
  GridNavigationConfig,
  LinearNavigationConfig,
} from "../core/navigation-types";
import type { NavigationRegionConfig } from "../core/navigation-hierarchy";

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

export const NavigationRegionContext =
  createContext<NavigationRegionConfig | null>(null);

export function useNavigationRegion(): NavigationRegionConfig | null {
  return useContext(NavigationRegionContext);
}

export const GridNavigationContext = createContext<GridNavigationConfig | null>(
  null,
);

export function useGridNavigation(): GridNavigationConfig | null {
  return useContext(GridNavigationContext);
}
