import { useMemo, type PropsWithChildren } from "react";

import type { NavigationRegionConfig } from "../core/navigation-hierarchy";
import { NavigationRegionContext } from "./linear-navigation-context";

export interface NavigationContentProps extends PropsWithChildren {
  navigationRegion: NavigationRegionConfig;
}

/**
 * Declares the focus region owned by the currently rendered content of a
 * navigation bar. Focusables below this component inherit the declaration,
 * including focusables inside feature-specific layouts.
 */
export function NavigationContent({
  navigationRegion,
  children,
}: NavigationContentProps) {
  const {
    childRegionId,
    entryFocusId,
    entryFocusPolicy,
    exitFocusId,
    gamepadExitFocusId,
    gamepadParentRegionId,
    parentRegionId,
    regionId,
  } = navigationRegion;
  const regionValue = useMemo(
    () => ({
      childRegionId,
      entryFocusId,
      entryFocusPolicy,
      exitFocusId,
      gamepadExitFocusId,
      gamepadParentRegionId,
      parentRegionId,
      regionId,
    }),
    [
      childRegionId,
      entryFocusId,
      entryFocusPolicy,
      exitFocusId,
      gamepadExitFocusId,
      gamepadParentRegionId,
      parentRegionId,
      regionId,
    ],
  );
  return (
    <NavigationRegionContext.Provider value={regionValue}>
      {children}
    </NavigationRegionContext.Provider>
  );
}
