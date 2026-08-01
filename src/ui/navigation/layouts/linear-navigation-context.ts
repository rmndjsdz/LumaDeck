import { createContext, useContext } from "react";

import type { LinearNavigationConfig } from "../core/navigation-types";

export const LinearNavigationContext =
  createContext<LinearNavigationConfig | null>(null);

export function useLinearNavigation(): LinearNavigationConfig | null {
  return useContext(LinearNavigationContext);
}
