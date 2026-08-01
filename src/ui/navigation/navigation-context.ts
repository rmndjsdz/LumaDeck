import { createContext, useContext } from "react";

import type { NavigationRuntime } from "./NavigationProvider";

export const NavigationContext = createContext<NavigationRuntime | null>(null);

export function useNavigation(): NavigationRuntime {
  const runtime = useContext(NavigationContext);
  if (!runtime) {
    throw new Error("useNavigation must be used inside NavigationProvider");
  }
  return runtime;
}
