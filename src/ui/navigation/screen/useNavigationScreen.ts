import { useLayoutEffect } from "react";

import { useNavigation } from "../navigation-context";
import type { NavigationScreenDefinition } from "./navigation-screen-contract";

export function useNavigationScreen(
  definition: NavigationScreenDefinition,
  active: boolean,
): void {
  const { engine } = useNavigation();
  const scopeId = definition.rootScope.scopeId;

  useLayoutEffect(() => {
    if (active) engine.notifyRouteActive(scopeId);
  }, [active, definition.route, engine, scopeId]);
}
