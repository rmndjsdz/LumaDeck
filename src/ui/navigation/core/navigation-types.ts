export type NavigationAction =
  | "move-up"
  | "move-down"
  | "move-left"
  | "move-right"
  | "confirm"
  | "back"
  | "menu"
  | "page-next"
  | "page-previous";

export type NavigationDirection = "up" | "down" | "left" | "right";

export type InputMode = "mouse" | "keyboard" | "gamepad";

export interface Rect {
  top: number;
  right: number;
  bottom: number;
  left: number;
  width: number;
  height: number;
}

export interface FocusNavigationOverrides {
  up?: string;
  down?: string;
  left?: string;
  right?: string;
}

export interface LinearNavigationConfig {
  groupId: string;
  axis: "horizontal" | "vertical";
  wrap?: boolean;
}

export interface FocusEntry {
  focusId: string;
  element: HTMLElement;
  scopeId: string;
  groupId?: string;
  disabled?: boolean;
  hidden?: boolean;
  navigation?: FocusNavigationOverrides;
  linearNavigation?: LinearNavigationConfig;
  onFocus?: () => void;
  onBlur?: () => void;
  onConfirm?: () => void;
  priority?: number;
}

export interface ScopeRegistration {
  scopeId: string;
  parentScopeId?: string;
  initialFocusId?: string;
  restoreFocus?: boolean;
  rememberScroll?: boolean;
  trapFocus?: boolean;
  modal?: boolean;
  activateOnMount?: boolean;
  onBack?: () => boolean | void;
}

export interface SpatialCandidate {
  focusId: string;
  rect: Rect;
  disabled?: boolean;
  hidden?: boolean;
  connected?: boolean;
  priority?: number;
}

export interface SpatialResolution {
  candidate: SpatialCandidate | null;
  evaluated: string[];
  durationMs: number;
}

export interface NavigationDebugState {
  registryCount: number;
  requestedDirection?: NavigationDirection;
  resolvedCandidate?: string;
  evaluatedCandidates: string[];
  resolutionTimeMs: number;
  lastRestoredFocus?: string;
  lastScroll?: string;
  gamepadConnected: boolean;
  actionsPerSecond: number;
  focusLosses: number;
  duplicateFocusIds: string[];
}
