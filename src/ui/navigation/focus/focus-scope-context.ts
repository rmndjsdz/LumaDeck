import { createContext, useContext } from "react";

export const FocusScopeContext = createContext<string | null>(null);

export function useFocusScopeId(): string | null {
  return useContext(FocusScopeContext);
}
