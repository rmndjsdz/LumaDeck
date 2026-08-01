export interface MouseAdapterOptions {
  onInputMode: () => void;
  movementThreshold?: number;
  target?: Window;
}

export class MouseAdapter {
  private readonly target: Window | null;
  private readonly onInputMode: () => void;
  private readonly movementThreshold: number;
  private lastPoint: { x: number; y: number } | null = null;
  private accumulatedDistance = 0;

  public constructor(options: MouseAdapterOptions) {
    this.target =
      options.target ?? (typeof window === "undefined" ? null : window);
    this.onInputMode = options.onInputMode;
    this.movementThreshold = options.movementThreshold ?? 8;
  }

  public start(): void {
    this.target?.addEventListener("pointermove", this.handlePointerMove);
    this.target?.addEventListener("wheel", this.handleWheel, { passive: true });
  }

  public stop(): void {
    this.target?.removeEventListener("pointermove", this.handlePointerMove);
    this.target?.removeEventListener("wheel", this.handleWheel);
    this.reset();
  }

  public dispose(): void {
    this.stop();
  }

  public markHover(): void {
    this.onInputMode();
  }

  public markClick(): void {
    this.onInputMode();
  }

  public handlePointerMove = (event: PointerEvent): void => {
    if (event.pointerType !== "mouse") return;
    if (!this.lastPoint) {
      this.lastPoint = { x: event.clientX, y: event.clientY };
      return;
    }
    const distance = Math.hypot(
      event.clientX - this.lastPoint.x,
      event.clientY - this.lastPoint.y,
    );
    this.lastPoint = { x: event.clientX, y: event.clientY };
    this.accumulatedDistance += distance;
    if (this.accumulatedDistance >= this.movementThreshold) {
      this.onInputMode();
      this.accumulatedDistance = 0;
    }
  };

  public handleWheel = (): void => {
    this.onInputMode();
  };

  private reset(): void {
    this.lastPoint = null;
    this.accumulatedDistance = 0;
  }
}
