import type { NavigationAction, NavigationDirection } from "./navigation-types";

export const NAVIGATION_ACTIONS: readonly NavigationAction[] = [
  "move-up",
  "move-down",
  "move-left",
  "move-right",
  "confirm",
  "back",
  "menu",
  "page-next",
  "page-previous",
  "previous-primary-screen",
  "next-primary-screen",
  "delete-character",
  "insert-space",
  "toggle-caps-lock",
  "shift-release",
  "accept-text",
];

export const DIRECTION_TO_ACTION: Record<
  NavigationDirection,
  NavigationAction
> = {
  up: "move-up",
  down: "move-down",
  left: "move-left",
  right: "move-right",
};

export const ACTION_TO_DIRECTION: Partial<
  Record<NavigationAction, NavigationDirection>
> = {
  "move-up": "up",
  "move-down": "down",
  "move-left": "left",
  "move-right": "right",
};

export function isNavigationAction(value: string): value is NavigationAction {
  return NAVIGATION_ACTIONS.includes(value as NavigationAction);
}

export function isDirectionAction(
  action: NavigationAction,
): action is "move-up" | "move-down" | "move-left" | "move-right" {
  return action in ACTION_TO_DIRECTION;
}
