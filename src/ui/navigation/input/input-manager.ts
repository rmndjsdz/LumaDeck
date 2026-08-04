import type { NavigationEngine } from "../core/navigation-engine";
import type { InputMode, NavigationAction } from "../core/navigation-types";
import { useNavigationStore } from "../../../stores/navigation-store";
import { DirectionRepeatController } from "./direction-repeat-controller";
import { GamepadAdapter } from "./gamepad-adapter";
import { KeyboardAdapter } from "./keyboard-adapter";
import { MouseAdapter } from "./mouse-adapter";
import { NavigationSettlingController } from "./navigation-settling";
import { markPerformance } from "../../performance/performance-marks";
import { navigationRuntimeTrace } from "../debug/navigation-runtime-trace";

export type SemanticActionHandler = (
  action: "previous-primary-screen" | "next-primary-screen",
  inputMode: InputMode,
) => boolean;

export class InputManager {
  private readonly repeatController = new DirectionRepeatController();
  private readonly keyboardAdapter: KeyboardAdapter;
  private readonly mouseAdapter: MouseAdapter;
  private readonly gamepadAdapter: GamepadAdapter;
  private started = false;
  private semanticActionHandler: SemanticActionHandler | null = null;
  private lastDiscreteAction: { action: NavigationAction; at: number } | null =
    null;
  private readonly settlingController: NavigationSettlingController;
  private inputFrozen = false;
  private rearming = false;
  private rearmTimer: ReturnType<typeof setTimeout> | null = null;

  public constructor(private readonly engine: NavigationEngine) {
    this.keyboardAdapter = new KeyboardAdapter({
      repeatController: this.repeatController,
      onAction: (action) => this.dispatch(action, "keyboard"),
      onTab: (shiftKey) => this.engine.handleTab(shiftKey),
      onInputMode: () => this.setInputMode("keyboard"),
      isInputBlocked: () => this.isInputBlocked(),
      getInputBlockReason: () => this.getInputBlockReason(),
    });
    this.mouseAdapter = new MouseAdapter({
      onInputMode: () => this.setInputMode("mouse"),
    });
    this.gamepadAdapter = new GamepadAdapter({
      repeatController: this.repeatController,
      onAction: (action) => this.dispatch(action, "gamepad"),
      onTriggerRelease: (action) => this.dispatch(action, "gamepad"),
      onInputMode: () => this.setInputMode("gamepad"),
      onConnectionChange: (connected) =>
        useNavigationStore.getState().setGamepadConnected(connected),
      onNeutral: () => this.handleInputNeutral(),
      onDiagnostic: (diagnostic) =>
        useNavigationStore.getState().setGamepadDiagnostic(diagnostic),
      isInputBlocked: () => this.isInputBlocked(),
    });
    this.settlingController = new NavigationSettlingController({
      onPhaseChange: (phase) => {
        useNavigationStore.getState().setNavigationPhase(phase);
      },
    });
  }

  public start(): void {
    if (this.started) return;
    this.started = true;
    this.keyboardAdapter.start();
    this.mouseAdapter.start();
    this.gamepadAdapter.start();
  }

  public stop(): void {
    if (!this.started) return;
    this.started = false;
    this.keyboardAdapter.stop();
    this.mouseAdapter.stop();
    this.gamepadAdapter.stop();
  }

  public dispose(): void {
    this.stop();
    this.repeatController.dispose();
    this.settlingController.dispose();
    this.clearRearmTimer();
  }

  public setSemanticActionHandler(
    handler: SemanticActionHandler | null,
  ): () => void {
    this.semanticActionHandler = handler;
    return () => {
      if (this.semanticActionHandler === handler) {
        this.semanticActionHandler = null;
      }
    };
  }

  public handlePointerHover(focusId: string): void {
    if (this.isInputBlocked()) return;
    this.mouseAdapter.markHover();
    this.engine.focusFromPointer(focusId);
  }

  public handlePointerConfirm(focusId: string): void {
    if (this.isInputBlocked()) return;
    this.mouseAdapter.markClick();
    if (!this.engine.focusFromPointer(focusId)) return;
    this.dispatch("confirm", "mouse");
  }

  public setInputMode(inputMode: InputMode): void {
    useNavigationStore.getState().setInputMode(inputMode);
  }

  public dispatch(action: NavigationAction, inputMode: InputMode): boolean {
    const blockReason = this.getInputBlockReason();
    if (blockReason) {
      navigationRuntimeTrace.record("INPUT_DISCARDED", {
        inputSource: inputMode,
        details: { reason: blockReason, action },
      });
      return true;
    }
    this.setInputMode(inputMode);
    markPerformance("input-received");
    if (action.startsWith("move-")) {
      this.settlingController.notifyNavigation();
    }
    const now = performance.now();
    if (
      (action === "confirm" || action === "back") &&
      this.lastDiscreteAction?.action === action &&
      now - this.lastDiscreteAction.at < 50
    ) {
      navigationRuntimeTrace.record("INPUT_DISCARDED", {
        inputSource: inputMode,
        details: { reason: "discrete-cooldown", action },
      });
      return false;
    }
    this.lastDiscreteAction = { action, at: now };
    navigationRuntimeTrace.record("INPUT_ACCEPTED", {
      inputSource: inputMode,
      details: { action },
    });
    const handled = this.engine.dispatch(action, inputMode);
    if (handled) return true;
    if (
      action === "previous-primary-screen" ||
      action === "next-primary-screen"
    ) {
      return this.semanticActionHandler?.(action, inputMode) ?? false;
    }
    if (action === "delete-character") {
      return this.engine.dispatch("back", inputMode);
    }
    if (action === "insert-space") {
      return this.engine.dispatch("confirm", inputMode);
    }
    if (action === "accept-text") {
      return this.engine.dispatch("confirm", inputMode);
    }
    return false;
  }

  public setInputFrozen(frozen: boolean): void {
    if (frozen === this.inputFrozen && !(frozen && this.rearming)) return;
    this.inputFrozen = frozen;
    this.clearRearmTimer();
    this.repeatController.stop();
    this.keyboardAdapter.handleBlur();
    this.gamepadAdapter.resetInputState();
    this.lastDiscreteAction = null;
    if (frozen) {
      this.rearming = false;
      return;
    }
    this.rearming = true;
  }

  public isInputFrozen(): boolean {
    return this.inputFrozen || this.rearming;
  }

  private isInputBlocked(): boolean {
    return this.inputFrozen || this.rearming;
  }

  private getInputBlockReason(): string | null {
    if (this.inputFrozen) return "input-frozen";
    if (this.rearming) return "rearming";
    return null;
  }

  private handleInputNeutral(): void {
    if (!this.rearming || this.inputFrozen || this.rearmTimer !== null) return;
    this.rearmTimer = setTimeout(() => {
      this.rearmTimer = null;
      this.rearming = false;
      this.keyboardAdapter.handleBlur();
      this.gamepadAdapter.resetInputState();
    }, 200);
  }

  private clearRearmTimer(): void {
    if (this.rearmTimer !== null) clearTimeout(this.rearmTimer);
    this.rearmTimer = null;
  }
}
