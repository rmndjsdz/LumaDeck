import { motionTokens } from "../motion/motion-tokens";
import { markPerformance } from "../performance/performance-marks";
import type { NavigationPhase } from "../navigation/core/navigation-types";

export interface BackgroundSnapshot {
  currentUrl: string | null;
  incomingUrl: string | null;
  incomingVisible: boolean;
}

export type BackgroundTelemetry =
  | { type: "request"; requestId: number; url: string }
  | { type: "decoded"; requestId: number; url: string; decodeTimeMs: number }
  | { type: "crossfade-started"; requestId: number }
  | { type: "crossfade-finished"; requestId: number }
  | { type: "cache-hit"; url: string }
  | { type: "cache-miss"; url: string }
  | { type: "pending"; pending: boolean }
  | { type: "cancelled"; requestId: number }
  | { type: "error"; requestId: number; url: string };

export interface BackgroundManagerOptions {
  durationMs?: number;
  imageFactory?: () => HTMLImageElement;
  reducedMotion?: () => boolean;
  maxCacheEntries?: number;
  onTelemetry?: (event: BackgroundTelemetry) => void;
}

type Listener = () => void;

interface CacheEntry {
  url: string;
  image: HTMLImageElement;
  promise: Promise<HTMLImageElement>;
  state: "pending" | "ready";
  lastUsed: number;
}

interface ActiveRequest {
  requestId: number;
  url: string;
  fallbackUrl: string | null;
}

const isDeferredPhase = (phase: NavigationPhase): boolean =>
  phase === "navigating" || phase === "fast-navigating";

export class BackgroundManager {
  private readonly durationMs: number;
  private readonly imageFactory: () => HTMLImageElement;
  private readonly reducedMotion: () => boolean;
  private readonly maxCacheEntries: number;
  private readonly onTelemetry?: (event: BackgroundTelemetry) => void;
  private readonly listeners = new Set<Listener>();
  private readonly cache = new Map<string, CacheEntry>();
  private snapshot: BackgroundSnapshot = {
    currentUrl: null,
    incomingUrl: null,
    incomingVisible: false,
  };
  private requestId = 0;
  private transitionTimer: number | null = null;
  private activeRequest: ActiveRequest | null = null;
  private phase: NavigationPhase = "idle";
  private cacheClock = 0;
  private disposed = false;

  public constructor(options: BackgroundManagerOptions = {}) {
    this.durationMs =
      options.durationMs ?? motionTokens.duration.backgroundCrossfade;
    this.imageFactory = options.imageFactory ?? (() => new Image());
    this.reducedMotion =
      options.reducedMotion ??
      (() =>
        typeof window !== "undefined" &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    this.maxCacheEntries = options.maxCacheEntries ?? 6;
    this.onTelemetry = options.onTelemetry;
  }

  public getSnapshot = (): BackgroundSnapshot => this.snapshot;

  public subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  public setNavigationPhase(phase: NavigationPhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
  }

  public request(
    url: string | null,
    phase: NavigationPhase = this.phase,
    fallbackUrl: string | null = null,
  ): void {
    if (!url) return;
    // React StrictMode replays effects during development. The external
    // manager can be disposed by the first replay cleanup and revived here.
    this.disposed = false;
    this.setNavigationPhase(phase);
    if (isDeferredPhase(phase)) {
      if (this.activeRequest) {
        this.emit({
          type: "cancelled",
          requestId: this.activeRequest.requestId,
        });
        this.activeRequest = null;
      }
      this.clearTransition();
      if (this.snapshot.incomingUrl !== null) {
        this.snapshot = {
          ...this.snapshot,
          incomingUrl: null,
          incomingVisible: false,
        };
        this.notify();
      }
      return;
    }
    if (url === this.snapshot.currentUrl) return;
    if (this.activeRequest?.url === url) return;
    this.startVisualRequest(url, fallbackUrl);
  }

  public preload(urls: readonly (string | null)[]): void {
    this.disposed = false;
    const uniqueUrls = [...new Set(urls)].filter(
      (url): url is string => Boolean(url) && url !== this.snapshot.currentUrl,
    );
    for (const url of uniqueUrls) {
      void this.ensureCached(url).catch(() => undefined);
    }
    this.trimCache();
  }

  public dispose(): void {
    this.disposed = true;
    this.clearTransition();
    this.listeners.clear();
    this.cache.clear();
    this.activeRequest = null;
  }

  private startVisualRequest(
    url: string,
    fallbackUrl: string | null = null,
  ): void {
    this.requestId += 1;
    const requestId = this.requestId;
    this.clearTransition();
    this.activeRequest = { requestId, url, fallbackUrl };
    this.snapshot = {
      ...this.snapshot,
      incomingUrl: url,
      incomingVisible: false,
    };
    this.emit({ type: "request", requestId, url });
    markPerformance("background-requested");
    this.notify();

    void this.ensureCached(url).catch(() => undefined);
  }

  private ensureCached(url: string): Promise<HTMLImageElement> {
    const existing = this.cache.get(url);
    if (existing) {
      existing.lastUsed = ++this.cacheClock;
      this.emit({ type: "cache-hit", url });
      if (existing.state === "ready") this.handleCacheReady(url);
      return existing.promise;
    }

    const image = this.imageFactory();
    const startedAt = performance.now();
    const entry: CacheEntry = {
      url,
      image,
      state: "pending",
      lastUsed: ++this.cacheClock,
      promise: Promise.resolve(image),
    };
    entry.promise = new Promise<HTMLImageElement>((resolve, reject) => {
      let settled = false;
      image.onload = () => {
        const finish = (): void => {
          if (settled) return;
          settled = true;
          entry.state = "ready";
          entry.lastUsed = ++this.cacheClock;
          const decodeTimeMs = performance.now() - startedAt;
          this.emit({
            type: "decoded",
            requestId: this.requestId,
            url,
            decodeTimeMs,
          });
          markPerformance("background-decoded");
          resolve(image);
          this.trimCache();
          if (
            ![...this.cache.values()].some((item) => item.state === "pending")
          ) {
            this.emit({ type: "pending", pending: false });
          }
          this.handleCacheReady(url);
        };
        const decode =
          typeof image.decode === "function" ? image.decode() : undefined;
        if (decode) {
          void decode.then(() => finish()).catch(reject);
        } else {
          finish();
        }
      };
      image.onerror = () => {
        if (settled) return;
        settled = true;
        this.cache.delete(url);
        this.handleCacheError(url);
        reject(new Error(`Background failed to load: ${url}`));
      };
      image.src = url;
      if (
        image.complete &&
        (image.naturalWidth > 0 || url.startsWith("data:"))
      ) {
        image.onload(new Event("load"));
      }
    });
    this.cache.set(url, entry);
    this.emit({ type: "cache-miss", url });
    this.emit({ type: "pending", pending: true });
    this.trimCache();
    return entry.promise;
  }

  private isCurrentRequest(requestId: number, url: string): boolean {
    return (
      !this.disposed &&
      this.activeRequest?.requestId === requestId &&
      this.activeRequest.url === url &&
      this.requestId === requestId
    );
  }

  private handleCacheReady(url: string): void {
    const activeRequest = this.activeRequest;
    if (!activeRequest || activeRequest.url !== url) return;
    const { requestId } = activeRequest;
    if (!this.isCurrentRequest(requestId, url)) return;
    if (this.phase !== "settling" && this.phase !== "idle") return;
    this.snapshot = { ...this.snapshot, incomingVisible: true };
    this.emit({ type: "crossfade-started", requestId });
    markPerformance("crossfade-started");
    this.notify();
    if (this.reducedMotion()) {
      this.commit(url, requestId);
      return;
    }
    this.transitionTimer = window.setTimeout(
      () => this.commit(url, requestId),
      this.durationMs,
    );
  }

  private handleCacheError(url: string): void {
    const activeRequest = this.activeRequest;
    if (!activeRequest || activeRequest.url !== url) return;
    if (!this.isCurrentRequest(activeRequest.requestId, url)) return;
    const fallbackUrl = activeRequest.fallbackUrl;
    this.activeRequest = null;
    if (fallbackUrl && fallbackUrl !== url) {
      this.startVisualRequest(fallbackUrl);
      return;
    }
    this.snapshot = {
      ...this.snapshot,
      incomingUrl: null,
      incomingVisible: false,
    };
    this.emit({ type: "error", requestId: activeRequest.requestId, url });
    this.notify();
  }

  private commit(url: string, requestId: number): void {
    if (!this.isCurrentRequest(requestId, url)) return;
    this.clearTransition();
    this.activeRequest = null;
    this.snapshot = {
      currentUrl: url,
      incomingUrl: null,
      incomingVisible: false,
    };
    this.emit({ type: "crossfade-finished", requestId });
    markPerformance("crossfade-finished");
    this.notify();
  }

  private clearTransition(): void {
    if (this.transitionTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.transitionTimer);
    }
    this.transitionTimer = null;
  }

  private trimCache(): void {
    while (this.cache.size > this.maxCacheEntries) {
      const protectedUrls = new Set([
        this.snapshot.currentUrl,
        this.snapshot.incomingUrl,
        this.activeRequest?.url ?? null,
      ]);
      const candidates = [...this.cache.values()].filter(
        (entry) => !protectedUrls.has(entry.url),
      );
      const oldest = candidates.sort((a, b) => a.lastUsed - b.lastUsed)[0];
      if (!oldest) return;
      this.cache.delete(oldest.url);
    }
  }

  private emit(event: BackgroundTelemetry): void {
    this.onTelemetry?.(event);
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
