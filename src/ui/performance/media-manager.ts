import type { Game } from "../../features/catalog/game-types";
import { recordMediaTiming, type MediaType } from "./media-timing";

export interface MediaDescriptor {
  gameId: string;
  mediaType: MediaType;
  url: string;
}

export type MediaResourceState = "idle" | "pending" | "ready" | "error";

export interface MediaResourceSnapshot {
  key: string;
  url: string;
  state: MediaResourceState;
  image: HTMLImageElement | null;
  version: number;
}

export interface MediaManagerEvent {
  type:
    | "MEDIA_CACHE_HIT"
    | "MEDIA_CACHE_MISS"
    | "MEDIA_CACHE_INSERT"
    | "MEDIA_CACHE_EVICT"
    | "MEDIA_PRELOAD_START"
    | "MEDIA_PRELOAD_READY"
    | "VISUAL_CACHE_HIT";
  descriptor?: MediaDescriptor;
  durationMs?: number;
  cacheSize: number;
  hotGames: number;
}

export interface MediaManagerOptions {
  imageFactory?: () => HTMLImageElement;
  maxEntries?: number;
  maxDetailsGames?: number;
  onEvent?: (event: MediaManagerEvent) => void;
}

interface MediaEntry {
  descriptor: MediaDescriptor;
  image: HTMLImageElement;
  state: Exclude<MediaResourceState, "idle">;
  promise: Promise<HTMLImageElement>;
  lastUsed: number;
  version: number;
  listeners: Set<() => void>;
}

const DEFAULT_MAX_ENTRIES = 512;
const DEFAULT_MAX_DETAILS_GAMES = 8;

export class MediaManager {
  private readonly imageFactory: () => HTMLImageElement;
  private readonly maxEntries: number;
  private readonly maxDetailsGames: number;
  private readonly onEvent?: (event: MediaManagerEvent) => void;
  private readonly entries = new Map<string, MediaEntry>();
  private readonly hotKeys = new Set<string>();
  private readonly homeHotKeys = new Set<string>();
  private readonly hotGames: string[] = [];
  private clock = 0;
  private disposed = false;

  public constructor(options: MediaManagerOptions = {}) {
    this.imageFactory = options.imageFactory ?? (() => new Image());
    this.maxEntries = options.maxEntries ?? DEFAULT_MAX_ENTRIES;
    this.maxDetailsGames = options.maxDetailsGames ?? DEFAULT_MAX_DETAILS_GAMES;
    this.onEvent = options.onEvent;
  }

  public getSnapshot = (url: string): MediaResourceSnapshot => {
    const entry = this.entries.get(url);
    if (!entry) {
      return { key: url, url, state: "idle", image: null, version: 0 };
    }
    entry.lastUsed = ++this.clock;
    return {
      key: url,
      url,
      state: entry.state,
      image: entry.state === "ready" ? entry.image : null,
      version: entry.version,
    };
  };

  public subscribe = (url: string, listener: () => void): (() => void) => {
    const entry = this.entries.get(url);
    if (entry) {
      entry.listeners.add(listener);
      return () => entry.listeners.delete(listener);
    }
    return () => undefined;
  };

  public ensure(descriptor: MediaDescriptor): Promise<HTMLImageElement> {
    this.disposed = false;
    const existing = this.entries.get(descriptor.url);
    if (existing) {
      existing.lastUsed = ++this.clock;
      if (existing.state === "ready") {
        this.emit("MEDIA_CACHE_HIT", descriptor);
        this.emit("VISUAL_CACHE_HIT", descriptor);
      }
      return existing.promise;
    }

    const image = this.imageFactory();
    const startedAt = performance.now();
    const entry: MediaEntry = {
      descriptor,
      image,
      state: "pending",
      promise: Promise.resolve(image),
      lastUsed: ++this.clock,
      version: 1,
      listeners: new Set(),
    };
    entry.promise = new Promise<HTMLImageElement>((resolve, reject) => {
      let settled = false;
      const notify = (): void => {
        for (const listener of entry.listeners) listener();
      };
      image.onload = () => {
        recordMediaTiming("IMG_LOAD", {
          gameId: descriptor.gameId,
          type: descriptor.mediaType,
          path: descriptor.url,
          durationMs: performance.now() - startedAt,
          detail: JSON.stringify({ source: "media-manager" }),
        });
        const finish = (): void => {
          if (settled) return;
          settled = true;
          entry.state = "ready";
          entry.version += 1;
          entry.lastUsed = ++this.clock;
          recordMediaTiming("IMG_DECODED", {
            gameId: descriptor.gameId,
            type: descriptor.mediaType,
            path: descriptor.url,
            durationMs: performance.now() - startedAt,
            detail: JSON.stringify({ source: "media-manager" }),
          });
          this.emit(
            "MEDIA_CACHE_INSERT",
            descriptor,
            performance.now() - startedAt,
          );
          notify();
          resolve(image);
          this.trim();
        };
        const decode =
          typeof image.decode === "function" ? image.decode() : undefined;
        if (decode) {
          void decode.then(finish).catch((error: unknown) => {
            if (settled) return;
            settled = true;
            entry.state = "error";
            entry.version += 1;
            notify();
            reject(error);
          });
        } else {
          finish();
        }
      };
      image.onerror = () => {
        if (settled) return;
        settled = true;
        entry.state = "error";
        entry.version += 1;
        recordMediaTiming("IMG_ERROR", {
          gameId: descriptor.gameId,
          type: descriptor.mediaType,
          path: descriptor.url,
          durationMs: performance.now() - startedAt,
          detail: JSON.stringify({ source: "media-manager" }),
        });
        notify();
        reject(new Error(`Media failed to load: ${descriptor.url}`));
        this.entries.delete(descriptor.url);
      };
      image.src = descriptor.url;
      if (
        image.complete &&
        (image.naturalWidth > 0 || descriptor.url.startsWith("data:"))
      ) {
        image.onload(new Event("load"));
      }
    });
    this.entries.set(descriptor.url, entry);
    this.emit("MEDIA_CACHE_MISS", descriptor);
    this.trim();
    return entry.promise;
  }

  public async preload(descriptors: readonly MediaDescriptor[]): Promise<void> {
    const unique = [
      ...new Map(
        descriptors.map((descriptor) => [descriptor.url, descriptor]),
      ).values(),
    ];
    if (unique.length === 0) return;
    const startedAt = performance.now();
    for (const descriptor of unique) {
      this.emit("MEDIA_PRELOAD_START", descriptor);
    }
    await Promise.all(
      unique.map((descriptor) =>
        this.ensure(descriptor)
          .then(() => undefined)
          .catch(() => undefined),
      ),
    );
    for (const descriptor of unique) {
      if (this.getSnapshot(descriptor.url).state === "ready") {
        this.emit(
          "MEDIA_PRELOAD_READY",
          descriptor,
          performance.now() - startedAt,
        );
      }
    }
  }

  public async preloadGame(
    game: Game,
    options: { includeScreenshots?: boolean } = {},
  ): Promise<void> {
    await this.preload(descriptorsForGame(game, options));
  }

  public touchDetailsGame(game: Game): void {
    const existingIndex = this.hotGames.indexOf(game.id);
    if (existingIndex >= 0) this.hotGames.splice(existingIndex, 1);
    this.hotGames.unshift(game.id);
    this.hotGames.splice(this.maxDetailsGames);
    this.rebuildHotKeys();
    for (const descriptor of descriptorsForGame(game)) {
      const entry = this.entries.get(descriptor.url);
      if (entry) entry.lastUsed = ++this.clock;
    }
    this.trim();
  }

  public setHomeHotset(games: readonly Game[]): void {
    for (const key of this.homeHotKeys) this.hotKeys.delete(key);
    this.homeHotKeys.clear();
    for (const game of games) {
      for (const descriptor of descriptorsForGame(game)) {
        // Home keeps its visible artwork hot; screenshots belong to Details.
        // The set is intentionally limited to the catalog's card/hero assets.
        if (descriptor.mediaType === "screenshot") continue;
        this.hotKeys.add(descriptor.url);
        this.homeHotKeys.add(descriptor.url);
      }
    }
    this.trim();
  }

  public getStats(): { entries: number; ready: number; hotGames: number } {
    return {
      entries: this.entries.size,
      ready: [...this.entries.values()].filter(
        (entry) => entry.state === "ready",
      ).length,
      hotGames: this.hotGames.length,
    };
  }

  public dispose(): void {
    this.disposed = true;
    for (const entry of this.entries.values()) {
      entry.listeners.clear();
    }
    this.entries.clear();
    this.hotKeys.clear();
    this.homeHotKeys.clear();
    this.hotGames.length = 0;
  }

  private rebuildHotKeys(): void {
    const retained = new Set(this.hotKeys);
    for (const entry of this.entries.values()) {
      if (
        entry.descriptor.gameId &&
        this.hotGames.includes(entry.descriptor.gameId)
      ) {
        retained.add(entry.descriptor.url);
      }
    }
    this.hotKeys.clear();
    for (const key of retained) this.hotKeys.add(key);
  }

  private trim(): void {
    if (this.disposed) return;
    while (this.entries.size > this.maxEntries) {
      const candidate = [...this.entries.values()]
        .filter(
          (entry) =>
            !this.hotKeys.has(entry.descriptor.url) &&
            !this.hotGames.includes(entry.descriptor.gameId),
        )
        .sort((left, right) => left.lastUsed - right.lastUsed)[0];
      if (!candidate) return;
      this.entries.delete(candidate.descriptor.url);
      this.emit("MEDIA_CACHE_EVICT", candidate.descriptor);
    }
  }

  private emit(
    type: MediaManagerEvent["type"],
    descriptor?: MediaDescriptor,
    durationMs?: number,
  ): void {
    const stats = this.getStats();
    const event: MediaManagerEvent = {
      type,
      descriptor,
      durationMs,
      cacheSize: stats.entries,
      hotGames: stats.hotGames,
    };
    this.onEvent?.(event);
    if (descriptor) {
      recordMediaTiming(type, {
        gameId: descriptor.gameId,
        type: descriptor.mediaType,
        path: descriptor.url,
        durationMs,
        detail: JSON.stringify({
          key: descriptor.url,
          state: this.entries.get(descriptor.url)?.state ?? "evicted",
          hotGames: this.hotGames.length,
          cacheSize: this.entries.size,
        }),
      });
    }
  }
}

export function descriptorsForGame(
  game: Game,
  options: { includeScreenshots?: boolean } = {},
): MediaDescriptor[] {
  const includeScreenshots = options.includeScreenshots ?? true;
  return [
    descriptor(game, "hero", game.backgroundUrl),
    descriptor(game, "logo", game.logoUrl),
    descriptor(game, "grid", game.coverUrl),
    descriptor(game, "grid", game.verticalCoverUrl),
    descriptor(game, "grid", game.squareCoverUrl ?? ""),
    descriptor(game, "grid", game.iconUrl ?? ""),
    ...(includeScreenshots
      ? game.screenshots.map((url) => descriptor(game, "screenshot", url))
      : []),
  ].filter((item): item is MediaDescriptor => Boolean(item.url));
}

function descriptor(
  game: Game,
  mediaType: MediaType,
  url: string,
): MediaDescriptor {
  return { gameId: game.id, mediaType, url };
}

export const mediaManager = new MediaManager({
  maxEntries: DEFAULT_MAX_ENTRIES,
  maxDetailsGames: DEFAULT_MAX_DETAILS_GAMES,
});
