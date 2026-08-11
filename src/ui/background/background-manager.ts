import { motionTokens } from "../motion/motion-tokens";
import { markPerformance } from "../performance/performance-marks";
import type { NavigationPhase } from "../navigation/core/navigation-types";
import { MediaManager } from "../performance/media-manager";

export interface BackgroundSnapshot {
  currentUrl: string | null;
  incomingUrl: string | null;
  incomingVisible: boolean;
}

export type BackgroundTelemetry =
  | { type: "request"; requestId: number; url: string; gameId?: string }
  | { type: "load"; requestId: number; url: string; gameId?: string }
  | {
      type: "decoded";
      requestId: number;
      url: string;
      decodeTimeMs: number;
      gameId?: string;
    }
  | { type: "crossfade-started"; requestId: number }
  | { type: "crossfade-finished"; requestId: number }
  | { type: "cache-hit"; url: string; gameId?: string }
  | { type: "cache-miss"; url: string; gameId?: string }
  | { type: "cache-evict"; url: string; gameId?: string }
  | { type: "pending"; pending: boolean }
  | { type: "cancelled"; requestId: number }
  | { type: "error"; requestId: number; url: string; gameId?: string };

export interface BackgroundManagerOptions {
  durationMs?: number;
  imageFactory?: () => HTMLImageElement;
  reducedMotion?: () => boolean;
  maxCacheEntries?: number;
  mediaManager?: MediaManager;
  onTelemetry?: (event: BackgroundTelemetry) => void;
}

type Listener = () => void;

interface CacheEntry {
  url: string;
  gameId?: string;
  promise: Promise<HTMLImageElement>;
  state: "pending" | "ready";
  lastUsed: number;
  unsubscribe: (() => void) | null;
}

interface ActiveRequest {
  requestId: number;
  url: string;
  fallbackUrl: string | null;
  gameId?: string;
}

const isDeferredPhase = (phase: NavigationPhase): boolean =>
  phase === "navigating" || phase === "fast-navigating";

export class BackgroundManager {
  private readonly durationMs: number;
  private readonly reducedMotion: () => boolean;
  private readonly maxCacheEntries: number;
  private readonly mediaManager: MediaManager;
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
    this.reducedMotion =
      options.reducedMotion ??
      (() =>
        typeof window !== "undefined" &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    this.maxCacheEntries = options.maxCacheEntries ?? 6;
    this.mediaManager =
      options.mediaManager ??
      new MediaManager({
        imageFactory: options.imageFactory,
        maxEntries: this.maxCacheEntries,
      });
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
    gameId?: string,
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
    this.startVisualRequest(url, fallbackUrl, gameId);
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
    for (const entry of this.cache.values()) entry.unsubscribe?.();
    this.cache.clear();
    this.activeRequest = null;
  }

  private startVisualRequest(
    url: string,
    fallbackUrl: string | null = null,
    gameId?: string,
  ): void {
    this.requestId += 1;
    const requestId = this.requestId;
    this.clearTransition();
    this.activeRequest = { requestId, url, fallbackUrl, gameId };
    this.snapshot = {
      ...this.snapshot,
      incomingUrl: url,
      incomingVisible: false,
    };
    this.emit({ type: "request", requestId, url, gameId });
    markPerformance("background-requested");
    this.notify();

    void this.ensureCached(url, gameId).catch(() => undefined);
  }

  private ensureCached(
    url: string,
    gameId?: string,
  ): Promise<HTMLImageElement> {
    const existing = this.cache.get(url);
    if (existing) {
      existing.lastUsed = ++this.cacheClock;
      this.emit({ type: "cache-hit", url, gameId: existing.gameId });
      if (existing.state === "ready") this.handleCacheReady(url);
      return existing.promise;
    }

    const startedAt = performance.now();
    const mediaPromise = this.mediaManager.ensure({
      gameId: gameId ?? "background",
      mediaType: "hero",
      url,
    });
    const entry: CacheEntry = {
      url,
      gameId,
      state: "pending",
      lastUsed: ++this.cacheClock,
      promise: mediaPromise,
      unsubscribe: null,
    };
    entry.promise = mediaPromise
      .then((image) => {
        this.emit({
          type: "load",
          requestId: this.requestId,
          url,
          gameId: entry.gameId,
        });
        entry.state = "ready";
        entry.lastUsed = ++this.cacheClock;
        const decodeTimeMs = performance.now() - startedAt;
        this.emit({
          type: "decoded",
          requestId: this.requestId,
          url,
          decodeTimeMs,
          gameId: entry.gameId,
        });
        markPerformance("background-decoded");
        this.trimCache();
        if (![...this.cache.values()].some((item) => item.state === "pending")) {
          this.emit({ type: "pending", pending: false });
        }
        this.handleCacheReady(url);
        return image;
      })
      .catch((error: unknown) => {
        this.cache.delete(url);
        entry.unsubscribe?.();
        entry.unsubscribe = null;
        this.handleCacheError(url);
        throw error;
      });
    this.cache.set(url, entry);
    entry.unsubscribe = this.mediaManager.subscribe(url, () => {
      const state = this.mediaManager.getSnapshot(url).state;
      if (state === "ready") {
        entry.state = "ready";
        entry.lastUsed = ++this.cacheClock;
        this.handleCacheReady(url);
      } else if (state === "error") {
        this.handleCacheError(url);
      }
    });
    this.emit({ type: "cache-miss", url, gameId });
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
    if (this.snapshot.incomingVisible) return;
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
      this.startVisualRequest(fallbackUrl, null, activeRequest.gameId);
      return;
    }
    this.snapshot = {
      ...this.snapshot,
      incomingUrl: null,
      incomingVisible: false,
    };
    this.emit({
      type: "error",
      requestId: activeRequest.requestId,
      url,
      gameId: activeRequest.gameId,
    });
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
      oldest.unsubscribe?.();
      oldest.unsubscribe = null;
      this.emit({
        type: "cache-evict",
        url: oldest.url,
        gameId: oldest.gameId,
      });
    }
  }

  private emit(event: BackgroundTelemetry): void {
    this.onTelemetry?.(event);
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
