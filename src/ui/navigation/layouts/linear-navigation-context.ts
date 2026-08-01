import { createContext, useContext } from "react";

import type {
  GridNavigationConfig,
  LinearNavigationConfig,
} from "../core/navigation-types";

export const LinearNavigationContext =
  createContext<LinearNavigationConfig | null>(null);

export function useLinearNavigation(): LinearNavigationConfig | null {
  return useContext(LinearNavigationContext);
}

export const GridNavigationContext = createContext<GridNavigationConfig | null>(
  null,
);

export function useGridNavigation(): GridNavigationConfig | null {
  return useContext(GridNavigationContext);
}
