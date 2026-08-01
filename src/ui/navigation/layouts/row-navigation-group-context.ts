import { createContext, useContext } from "react";

export interface RowNavigationGroupContextValue {
  groupId: string;
  scopeId: string;
  orientation: "vertical";
  preserveHorizontalIntent: boolean;
}

export const RowNavigationGroupContext =
  createContext<RowNavigationGroupContextValue | null>(null);

export function useRowNavigationGroup(): RowNavigationGroupContextValue | null {
  return useContext(RowNavigationGroupContext);
}
