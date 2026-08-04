import type { NavigationRegionConfig } from "../core/navigation-hierarchy";
import type { ScopeActionHandler } from "../core/navigation-types";

export interface NavigationScreenRowGroupDefinition {
  groupId: string;
  orientation: "vertical" | "horizontal";
  preserveHorizontalIntent?: boolean;
}

export interface NavigationScreenDefinition {
  id: string;
  route: string;
  rootScope: {
    scopeId: string;
    parentScopeId?: string;
  };
  initialFocus: string;
  regions: readonly NavigationRegionConfig[];
  rowGroups: readonly NavigationScreenRowGroupDefinition[];
  restorePolicy: {
    restoreFocus: boolean;
    rememberScroll: boolean;
  };
  onBack?: () => boolean | void;
  onAction?: ScopeActionHandler;
}
