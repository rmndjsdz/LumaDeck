import {
  ACTION_TO_DIRECTION,
  DIRECTION_TO_ACTION,
} from "../core/navigation-actions";
import type { NavigationAction } from "../core/navigation-types";
import { DirectionRepeatController } from "./direction-repeat-controller";
import { navigationRuntimeTrace } from "../debug/navigation-runtime-trace";

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
  " ": "insert-space",
  Space: "insert-space",
  Escape: "back",
  Backspace: "delete-character",
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
  onTab?: (shiftKey: boolean) => boolean;
  onInputMode: () => void;
  repeatController: DirectionRepeatController;
  isInputBlocked?: () => boolean;
  getInputBlockReason?: () => string | null;
  target?: Window;
}

export class KeyboardAdapter {
  private readonly target: Window | null;
  private readonly onAction: (action: NavigationAction) => void;
  private readonly onTab?: (shiftKey: boolean) => boolean;
  private readonly onInputMode: () => void;
  private readonly repeatController: DirectionRepeatController;
  private readonly isInputBlocked: () => boolean;
  private readonly getInputBlockReason: () => string | null;
  private activeKey: string | null = null;

  public constructor(options: KeyboardAdapterOptions) {
    this.target =
      options.target ?? (typeof window === "undefined" ? null : window);
    this.onAction = options.onAction;
    this.onTab = options.onTab;
    this.onInputMode = options.onInputMode;
    this.repeatController = options.repeatController;
    this.isInputBlocked = options.isInputBlocked ?? (() => false);
    this.getInputBlockReason = options.getInputBlockReason ?? (() => null);
  }

  public start(): void {
    navigationRuntimeTrace.record("KEYBOARD_ADAPTER_STARTED", {
      details: { target: "window" },
    });
    this.target?.addEventListener("keydown", this.handleKeyDown);
    this.target?.addEventListener("keyup", this.handleKeyUp);
    this.target?.addEventListener("blur", this.handleBlur);
  }

  public stop(): void {
    this.target?.removeEventListener("keydown", this.handleKeyDown);
    this.target?.removeEventListener("keyup", this.handleKeyUp);
    this.target?.removeEventListener("blur", this.handleBlur);
    this.handleBlur();
    navigationRuntimeTrace.record("KEYBOARD_ADAPTER_STOPPED", {
      details: { target: "window" },
    });
  }

  public dispose(): void {
    this.stop();
  }

  public handleKeyDown = (event: KeyboardEvent): void => {
    const editableTarget = isEditableTarget(event.target);
    if (editableTarget) {
      if (event.key.startsWith("Arrow")) {
        navigationRuntimeTrace.record("INPUT_DISCARDED", {
          details: { reason: "editable-target", key: event.key },
        });
      }
      return;
    }
    if (event.key === "Tab") {
      if (this.isInputBlocked()) {
        event.preventDefault();
        return;
      }
      if (this.onTab?.(event.shiftKey)) {
        this.onInputMode();
        event.preventDefault();
      }
      return;
    }
    const action = KEYBOARD_ACTION_MAP[event.key];
    if (!action) return;
    navigationRuntimeTrace.record("KEYBOARD_INTENT_CREATED", {
      inputSource: "keyboard",
      details: {
        key: event.key,
        action,
        repeat: event.repeat,
        targetTag:
          event.target instanceof HTMLElement
            ? event.target.tagName.toLowerCase()
            : null,
      },
    });
    if (this.isInputBlocked()) {
      event.preventDefault();
      navigationRuntimeTrace.record("INPUT_DISCARDED", {
        inputSource: "keyboard",
        details: {
          reason: this.getInputBlockReason() ?? "input-blocked",
          key: event.key,
          action,
        },
      });
      this.handleBlur();
      return;
    }
    if (event.repeat) {
      navigationRuntimeTrace.record("INPUT_DISCARDED", {
        inputSource: "keyboard",
        details: { reason: "event-repeat", key: event.key, action },
      });
      return;
    }
    this.onInputMode();
    event.preventDefault();
    this.onAction(event.key === "Enter" ? "accept-text" : action);

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
