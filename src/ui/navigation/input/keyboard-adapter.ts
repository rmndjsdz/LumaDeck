import {
  ACTION_TO_DIRECTION,
  DIRECTION_TO_ACTION,
} from "../core/navigation-actions";
import type { NavigationAction } from "../core/navigation-types";
import { DirectionRepeatController } from "./direction-repeat-controller";

export const KEYBOARD_ACTION_MAP: Readonly<Record<string, NavigationAction>> = {
  ArrowUp: "move-up",
  w: "move-up",
  W: "move-up",
  ArrowDown: "move-down",
  s: "move-down",
  S: "move-down",
  ArrowLeft: "move-left",
  a: "move-left",
  A: "move-left",
  ArrowRight: "move-right",
  d: "move-right",
  D: "move-right",
  Enter: "confirm",
  " ": "confirm",
  Space: "confirm",
  Escape: "back",
  Backspace: "back",
  PageUp: "page-previous",
  PageDown: "page-next",
};

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable,
  );
}

interface KeyboardAdapterOptions {
  onAction: (action: NavigationAction) => void;
  onInputMode: () => void;
  repeatController: DirectionRepeatController;
  target?: Window;
}

export class KeyboardAdapter {
  private readonly target: Window | null;
  private readonly onAction: (action: NavigationAction) => void;
  private readonly onInputMode: () => void;
  private readonly repeatController: DirectionRepeatController;
  private activeKey: string | null = null;

  public constructor(options: KeyboardAdapterOptions) {
    this.target =
      options.target ?? (typeof window === "undefined" ? null : window);
    this.onAction = options.onAction;
    this.onInputMode = options.onInputMode;
    this.repeatController = options.repeatController;
  }

  public start(): void {
    this.target?.addEventListener("keydown", this.handleKeyDown);
    this.target?.addEventListener("keyup", this.handleKeyUp);
    this.target?.addEventListener("blur", this.handleBlur);
  }

  public stop(): void {
    this.target?.removeEventListener("keydown", this.handleKeyDown);
    this.target?.removeEventListener("keyup", this.handleKeyUp);
    this.target?.removeEventListener("blur", this.handleBlur);
    this.handleBlur();
  }

  public dispose(): void {
    this.stop();
  }

  public handleKeyDown = (event: KeyboardEvent): void => {
    if (isEditableTarget(event.target)) return;
    const action = KEYBOARD_ACTION_MAP[event.key];
    if (!action || event.repeat) return;
    this.onInputMode();
    event.preventDefault();
    this.onAction(action);

    const direction = ACTION_TO_DIRECTION[action];
    if (direction) {
      this.activeKey = event.key;
      this.repeatController.start(direction, () => {
        this.onAction(DIRECTION_TO_ACTION[direction]);
      });
    }
  };

  public handleKeyUp = (event: KeyboardEvent): void => {
    if (event.key === this.activeKey) this.handleBlur();
  };

  public handleBlur = (): void => {
    this.activeKey = null;
    this.repeatController.stop();
  };
}
