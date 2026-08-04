import type {
  InputMode,
  NavigationAction,
  PrimaryNavigationBlockReason,
} from "../core/navigation-types";

export interface PrimaryScreenDefinition<ScreenId extends string> {
  id: ScreenId;
  enabled?: boolean;
  initialFocusId?: string;
}

export type PrimaryScreenDirection = "previous" | "next";
export type PrimaryScreenTrigger = "left" | "right";
export type PrimaryScreenIgnoredReason =
  | "edge"
  | "modal"
  | "transition-pending"
  | "restoration-pending"
  | "unknown-screen"
  | "disabled";

export type PrimaryScreenTraceRecord<ScreenId extends string> =
  | {
      event: "PRIMARY_SCREEN_INPUT";
      source: InputMode;
      trigger: PrimaryScreenTrigger;
    }
  | {
      event: "PRIMARY_SCREEN_RESOLVE";
      currentScreen: ScreenId;
      direction: PrimaryScreenDirection;
      targetScreen: ScreenId | null;
    }
  | {
      event: "PRIMARY_SCREEN_TRANSITION_REQUEST";
      source: "trigger-navigation";
      targetScreen: ScreenId;
    }
  | {
      event: "PRIMARY_SCREEN_INPUT_IGNORED";
      reason: PrimaryScreenIgnoredReason;
      source: InputMode;
      trigger: PrimaryScreenTrigger;
    };

export interface PrimaryScreenNavigatorOptions<ScreenId extends string> {
  screens: readonly PrimaryScreenDefinition<ScreenId>[];
  getCurrentScreen: () => string | null;
  getBlockReason?: () => PrimaryNavigationBlockReason | null;
  onTransitionRequest: (
    targetScreen: ScreenId,
    direction: PrimaryScreenDirection,
  ) => void;
  onTrace?: (record: PrimaryScreenTraceRecord<ScreenId>) => void;
}

export class PrimaryScreenNavigator<ScreenId extends string> {
  private readonly screens: readonly PrimaryScreenDefinition<ScreenId>[];
  private readonly getCurrentScreen: () => string | null;
  private readonly getBlockReason: () => PrimaryNavigationBlockReason | null;
  private readonly onTransitionRequest: (
    targetScreen: ScreenId,
    direction: PrimaryScreenDirection,
  ) => void;
  private readonly onTrace?: (
    record: PrimaryScreenTraceRecord<ScreenId>,
  ) => void;
  private readonly records: PrimaryScreenTraceRecord<ScreenId>[] = [];

  public constructor(options: PrimaryScreenNavigatorOptions<ScreenId>) {
    this.screens = options.screens;
    this.getCurrentScreen = options.getCurrentScreen;
    this.getBlockReason = options.getBlockReason ?? (() => null);
    this.onTransitionRequest = options.onTransitionRequest;
    this.onTrace = options.onTrace;
  }

  public handle(
    action: Extract<
      NavigationAction,
      "previous-primary-screen" | "next-primary-screen"
    >,
    source: InputMode,
  ): boolean {
    const direction: PrimaryScreenDirection =
      action === "previous-primary-screen" ? "previous" : "next";
    const trigger: PrimaryScreenTrigger =
      direction === "previous" ? "left" : "right";
    this.emit({ event: "PRIMARY_SCREEN_INPUT", source, trigger });

    const blockReason = this.getBlockReason();
    if (blockReason) {
      this.emit({
        event: "PRIMARY_SCREEN_INPUT_IGNORED",
        reason: blockReason,
        source,
        trigger,
      });
      return false;
    }

    const currentScreen = this.getCurrentScreen();
    const currentIndex = this.screens.findIndex(
      (screen) => screen.id === currentScreen,
    );
    if (currentIndex < 0 || !currentScreen) {
      this.emit({
        event: "PRIMARY_SCREEN_INPUT_IGNORED",
        reason: "unknown-screen",
        source,
        trigger,
      });
      return false;
    }
    const currentScreenId = this.screens[currentIndex]?.id;
    if (!currentScreenId) return false;

    const targetIndex = currentIndex + (direction === "previous" ? -1 : 1);
    const target = this.screens[targetIndex];
    const targetScreen = target?.id ?? null;
    this.emit({
      event: "PRIMARY_SCREEN_RESOLVE",
      currentScreen: currentScreenId,
      direction,
      targetScreen,
    });

    if (!target) {
      this.emit({
        event: "PRIMARY_SCREEN_INPUT_IGNORED",
        reason: "edge",
        source,
        trigger,
      });
      return false;
    }
    if (target.enabled === false) {
      this.emit({
        event: "PRIMARY_SCREEN_INPUT_IGNORED",
        reason: "disabled",
        source,
        trigger,
      });
      return false;
    }

    this.emit({
      event: "PRIMARY_SCREEN_TRANSITION_REQUEST",
      source: "trigger-navigation",
      targetScreen: target.id,
    });
    this.onTransitionRequest(target.id, direction);
    return true;
  }

  public getTrace(): PrimaryScreenTraceRecord<ScreenId>[] {
    return [...this.records];
  }

  private emit(record: PrimaryScreenTraceRecord<ScreenId>): void {
    this.records.push(record);
    this.onTrace?.(record);
  }
}
