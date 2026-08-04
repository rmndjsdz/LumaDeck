import { DIRECTION_TO_ACTION } from "../core/navigation-actions";
import type {
  GamepadDiagnostic,
  NavigationAction,
  NavigationDirection,
} from "../core/navigation-types";
import { DirectionRepeatController } from "./direction-repeat-controller";

export type GamepadStateSource = () => readonly (Gamepad | null)[];

interface GamepadAdapterOptions {
  onAction: (action: NavigationAction) => void;
  onTriggerRelease?: (action: NavigationAction) => void;
  onInputMode: () => void;
  onConnectionChange: (connected: boolean) => void;
  onNeutral?: () => void;
  onDiagnostic?: (diagnostic: GamepadDiagnostic | undefined) => void;
  isInputBlocked?: () => boolean;
  repeatController: DirectionRepeatController;
  deadzone?: number;
  triggerPressThreshold?: number;
  triggerReleaseThreshold?: number;
  source?: GamepadStateSource;
  target?: Window;
  requestFrame?: (callback: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

export class GamepadAdapter {
  private readonly target: Window | null;
  private readonly onAction: (action: NavigationAction) => void;
  private readonly onTriggerRelease?: (action: NavigationAction) => void;
  private readonly onInputMode: () => void;
  private readonly onConnectionChange: (connected: boolean) => void;
  private readonly onNeutral?: () => void;
  private readonly onDiagnostic?: (
    diagnostic: GamepadDiagnostic | undefined,
  ) => void;
  private readonly isInputBlocked: () => boolean;
  private readonly repeatController: DirectionRepeatController;
  private readonly deadzone: number;
  private readonly triggerPressThreshold: number;
  private readonly triggerReleaseThreshold: number;
  private readonly source: GamepadStateSource;
  private readonly requestFrame: (callback: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private readonly previousButtons = new Map<number, boolean>();
  private readonly previousTriggers = new Map<number, boolean>();
  private frameHandle: number | null = null;
  private running = false;
  private connected = false;
  private currentDirection: NavigationDirection | null = null;
  private lastDiagnosticKey = "";

  public constructor(options: GamepadAdapterOptions) {
    this.target =
      options.target ?? (typeof window === "undefined" ? null : window);
    this.onAction = options.onAction;
    this.onTriggerRelease = options.onTriggerRelease;
    this.onInputMode = options.onInputMode;
    this.onConnectionChange = options.onConnectionChange;
    this.onNeutral = options.onNeutral;
    this.onDiagnostic = options.onDiagnostic;
    this.isInputBlocked = options.isInputBlocked ?? (() => false);
    this.repeatController = options.repeatController;
    this.deadzone = options.deadzone ?? 0.35;
    this.triggerPressThreshold = options.triggerPressThreshold ?? 0.75;
    this.triggerReleaseThreshold = options.triggerReleaseThreshold ?? 0.55;
    this.source =
      options.source ??
      (() =>
        typeof navigator === "undefined" ||
        typeof navigator.getGamepads !== "function"
          ? []
          : navigator.getGamepads());
    this.requestFrame =
      options.requestFrame ??
      ((callback) => {
        if (typeof window === "undefined") return 0;
        return window.setTimeout(() => callback(performance.now()), 16);
      });
    this.cancelFrame =
      options.cancelFrame ??
      ((handle) => {
        if (typeof window !== "undefined") window.clearTimeout(handle);
      });
  }

  public start(): void {
    if (this.running) return;
    this.running = true;
    this.target?.addEventListener("gamepadconnected", this.handleConnection);
    this.target?.addEventListener("gamepaddisconnected", this.handleConnection);
    this.poll();
  }

  public stop(): void {
    this.running = false;
    if (this.frameHandle !== null) this.cancelFrame(this.frameHandle);
    this.frameHandle = null;
    this.repeatController.stop();
    this.currentDirection = null;
    this.lastDiagnosticKey = "";
    this.previousButtons.clear();
    this.previousTriggers.clear();
    this.target?.removeEventListener("gamepadconnected", this.handleConnection);
    this.target?.removeEventListener(
      "gamepaddisconnected",
      this.handleConnection,
    );
    if (this.connected) this.onConnectionChange(false);
    this.onDiagnostic?.(undefined);
    this.connected = false;
  }

  public dispose(): void {
    this.stop();
  }

  public resetInputState(): void {
    this.repeatController.stop();
    this.currentDirection = null;
    this.previousButtons.clear();
    this.previousTriggers.clear();
  }

  public poll = (): void => {
    if (!this.running) return;
    this.frameHandle = null;
    const gamepad = this.source().find((candidate) => candidate?.connected);
    const isConnected = Boolean(gamepad);
    if (isConnected !== this.connected) {
      this.connected = isConnected;
      this.onConnectionChange(isConnected);
    }
    if (!gamepad) {
      this.onNeutral?.();
      this.frameHandle = this.requestFrame(this.poll);
      return;
    }

    this.onInputMode();
    const direction = this.readDirection(gamepad);
    this.handleButtons(gamepad);
    this.handleTriggers(gamepad);
    this.handleDirection(direction);
    if (
      !direction &&
      Array.from(gamepad.buttons).every((button) => !button?.pressed)
    ) {
      this.onNeutral?.();
    }
    this.publishDiagnostic(gamepad, direction);
    this.frameHandle = this.requestFrame(this.poll);
  };

  private handleButtons(gamepad: Gamepad): void {
    const buttonActions: Readonly<Record<number, NavigationAction>> = {
      0: "confirm",
      1: "back",
      2: "delete-character",
      3: "insert-space",
      4: "page-previous",
      5: "page-next",
      11: "toggle-caps-lock",
    };
    for (const [indexText, action] of Object.entries(buttonActions)) {
      const index = Number(indexText);
      const pressed = gamepad.buttons[index]?.pressed ?? false;
      const wasPressed = this.previousButtons.get(index) ?? false;
      if (pressed && !wasPressed && !this.isInputBlocked()) {
        this.onAction(action);
      }
      this.previousButtons.set(index, pressed);
    }
  }

  private handleTriggers(gamepad: Gamepad): void {
    const triggers: ReadonlyArray<{
      buttonIndex: number;
      axisIndex: number;
      action: NavigationAction;
      releaseAction?: NavigationAction;
    }> = [
      {
        buttonIndex: 6,
        axisIndex: 2,
        action: "previous-primary-screen",
        releaseAction: "shift-release",
      },
      {
        buttonIndex: 7,
        axisIndex: 5,
        action: "next-primary-screen",
      },
    ];

    for (const trigger of triggers) {
      const value = this.readTriggerValue(
        gamepad,
        trigger.buttonIndex,
        trigger.axisIndex,
      );
      const wasPressed =
        this.previousTriggers.get(trigger.buttonIndex) ?? false;
      const pressed = value >= this.triggerPressThreshold;
      if (pressed && !wasPressed && !this.isInputBlocked()) {
        this.onAction(trigger.action);
      }
      if (
        wasPressed &&
        !pressed &&
        value <= this.triggerReleaseThreshold &&
        trigger.releaseAction
      ) {
        this.onTriggerRelease?.(trigger.releaseAction);
      }
      if (!pressed && value <= this.triggerReleaseThreshold) {
        this.previousTriggers.set(trigger.buttonIndex, false);
      } else if (pressed) {
        this.previousTriggers.set(trigger.buttonIndex, true);
      }
    }
  }

  private readTriggerValue(
    gamepad: Gamepad,
    buttonIndex: number,
    axisIndex: number,
  ): number {
    const button = gamepad.buttons[buttonIndex];
    if (button && (button.value > 0 || button.pressed)) {
      return button.pressed ? Math.max(button.value, 1) : button.value;
    }
    return Math.abs(gamepad.axes[axisIndex] ?? 0);
  }

  private handleDirection(direction: NavigationDirection | null): void {
    if (direction === this.currentDirection) return;
    this.currentDirection = direction;
    this.repeatController.stop();
    if (!direction) return;
    if (this.isInputBlocked()) return;
    this.onAction(DIRECTION_TO_ACTION[direction]);
    this.repeatController.start(direction, () => {
      this.onAction(DIRECTION_TO_ACTION[direction]);
    });
  }

  private publishDiagnostic(
    gamepad: Gamepad,
    direction: NavigationDirection | null,
  ): void {
    if (!this.onDiagnostic) return;
    const pressedButtons = Array.from(gamepad.buttons).reduce<number[]>(
      (pressed, button, index) => {
        if (button?.pressed) pressed.push(index);
        return pressed;
      },
      [],
    );
    const key = `${gamepad.id}|${direction ?? "none"}|${pressedButtons.join(",")}`;
    if (key === this.lastDiagnosticKey) return;
    this.lastDiagnosticKey = key;
    this.onDiagnostic({ id: gamepad.id, direction, pressedButtons });
  }

  private readDirection(gamepad: Gamepad): NavigationDirection | null {
    const dpad: Readonly<Record<number, NavigationDirection>> = {
      12: "up",
      13: "down",
      14: "left",
      15: "right",
    };
    for (const [indexText, direction] of Object.entries(dpad)) {
      if (gamepad.buttons[Number(indexText)]?.pressed) return direction;
    }

    const x = gamepad.axes[0] ?? 0;
    const y = gamepad.axes[1] ?? 0;
    if (Math.max(Math.abs(x), Math.abs(y)) < this.deadzone) return null;
    if (Math.abs(x) >= Math.abs(y)) return x < 0 ? "left" : "right";
    return y < 0 ? "up" : "down";
  }

  private handleConnection = (): void => {
    if (this.running && this.frameHandle === null) this.poll();
  };
}
