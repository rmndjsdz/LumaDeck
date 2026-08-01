export interface BackgroundSnapshot {
  currentUrl: string | null;
  incomingUrl: string | null;
  incomingVisible: boolean;
}

export interface BackgroundManagerOptions {
  durationMs?: number;
  imageFactory?: () => HTMLImageElement;
  reducedMotion?: () => boolean;
}

type Listener = () => void;

export class BackgroundManager {
  private readonly durationMs: number;
  private readonly imageFactory: () => HTMLImageElement;
  private readonly reducedMotion: () => boolean;
  private readonly listeners = new Set<Listener>();
  private snapshot: BackgroundSnapshot = {
    currentUrl: null,
    incomingUrl: null,
    incomingVisible: false,
  };
  private requestId = 0;
  private transitionTimer: number | null = null;
  private activeImage: HTMLImageElement | null = null;

  public constructor(options: BackgroundManagerOptions = {}) {
    this.durationMs = options.durationMs ?? 240;
    this.imageFactory = options.imageFactory ?? (() => new Image());
    this.reducedMotion =
      options.reducedMotion ??
      (() =>
        typeof window !== "undefined" &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }

  public getSnapshot = (): BackgroundSnapshot => this.snapshot;

  public subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  public request(url: string | null): void {
    if (!url || url === this.snapshot.currentUrl) return;
    this.requestId += 1;
    const requestId = this.requestId;
    this.cancelPending();
    const image = this.imageFactory();
    this.activeImage = image;
    this.snapshot = {
      ...this.snapshot,
      incomingUrl: url,
      incomingVisible: false,
    };
    this.notify();
    image.onload = () => {
      if (requestId !== this.requestId) return;
      this.snapshot = { ...this.snapshot, incomingVisible: true };
      this.notify();
      if (this.reducedMotion()) {
        this.commit(url);
        return;
      }
      this.transitionTimer = window.setTimeout(
        () => this.commit(url),
        this.durationMs,
      );
    };
    image.onerror = () => {
      if (requestId !== this.requestId) return;
      this.snapshot = {
        ...this.snapshot,
        incomingUrl: null,
        incomingVisible: false,
      };
      this.notify();
    };
    image.src = url;
  }

  public preload(urls: readonly (string | null)[]): void {
    for (const url of urls) {
      if (!url || url === this.snapshot.currentUrl) continue;
      const image = this.imageFactory();
      image.src = url;
    }
  }

  public dispose(): void {
    this.cancelPending();
    this.listeners.clear();
  }

  private commit(url: string): void {
    this.transitionTimer = null;
    this.snapshot = {
      currentUrl: url,
      incomingUrl: null,
      incomingVisible: false,
    };
    this.notify();
  }

  private cancelPending(): void {
    if (this.transitionTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.transitionTimer);
    }
    this.transitionTimer = null;
    if (this.activeImage) {
      this.activeImage.onload = null;
      this.activeImage.onerror = null;
    }
    this.activeImage = null;
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
