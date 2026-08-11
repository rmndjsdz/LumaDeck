import { useEffect, useMemo, useSyncExternalStore } from "react";
import type { Game } from "../../features/catalog/game-types";
import { useNavigationStore } from "../../stores/navigation-store";
import { getGameBackgroundUrls } from "../../features/catalog/game-media";
import {
  BackgroundManager,
  type BackgroundTelemetry,
} from "./background-manager";
import { recordMediaTiming } from "../performance/media-timing";
import { mediaManager } from "../performance/media-manager";
import { MediaImage } from "../performance/MediaImage";

interface BackgroundViewProps {
  games: Game[];
  fallbackGameId: string | null;
}

export function BackgroundView({ games, fallbackGameId }: BackgroundViewProps) {
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const navigationPhase = useNavigationStore((state) => state.navigationPhase);
  const manager = useMemo(
    () =>
      new BackgroundManager({
        mediaManager,
        onTelemetry: handleTelemetry,
      }),
    [],
  );
  const snapshot = useSyncExternalStore(
    manager.subscribe,
    manager.getSnapshot,
    manager.getSnapshot,
  );
  const targetGame = useMemo(() => {
    const focusedGame = games.find((game) =>
      Boolean(activeFocusId && activeFocusId.endsWith(game.id)),
    );
    return (
      focusedGame ??
      games.find((game) => game.id === fallbackGameId) ??
      games.find((game) => game.status === "playing") ??
      games[0]
    );
  }, [activeFocusId, fallbackGameId, games]);
  const preloadUrls = useMemo(() => {
    if (!targetGame) return [];
    const index = games.findIndex((game) => game.id === targetGame.id);
    return [
      ...(games[index - 1] ? getGameBackgroundUrls(games[index - 1]) : []),
      ...(games[index + 1] ? getGameBackgroundUrls(games[index + 1]) : []),
      ...(games[index + 2] ? getGameBackgroundUrls(games[index + 2]) : []),
    ];
  }, [games, targetGame]);

  const targetBackgroundUrls = useMemo(
    () => (targetGame ? getGameBackgroundUrls(targetGame) : []),
    [targetGame],
  );

  useEffect(() => {
    manager.request(
      targetBackgroundUrls[0] ?? null,
      navigationPhase,
      targetBackgroundUrls[1] ?? null,
      targetGame?.id,
    );
    manager.preload(preloadUrls);
  }, [
    manager,
    navigationPhase,
    preloadUrls,
    targetBackgroundUrls,
    targetGame?.id,
  ]);

  useEffect(() => () => manager.dispose(), [manager]);

  return (
    <div className="background-view" aria-hidden="true">
      <div className="background-layer background-layer-current">
        <MediaImage
          gameId={targetGame?.id ?? fallbackGameId ?? "background"}
          mediaType="hero"
          src={snapshot.currentUrl ?? undefined}
          alt=""
          aria-hidden="true"
          className="background-image"
        />
      </div>
      <div
        className={`background-layer background-layer-incoming${snapshot.incomingVisible ? " is-visible" : ""}`}
      >
        <MediaImage
          gameId={targetGame?.id ?? fallbackGameId ?? "background"}
          mediaType="hero"
          src={snapshot.incomingUrl ?? undefined}
          alt=""
          aria-hidden="true"
          className="background-image"
        />
      </div>
      <div className="background-vignette" />
    </div>
  );
}

function handleTelemetry(event: BackgroundTelemetry): void {
  if (event.type === "request" && event.gameId) {
    recordMediaTiming("IMG_REQUEST", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  } else if (event.type === "load" && event.gameId) {
    recordMediaTiming("IMG_LOAD", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  } else if (event.type === "decoded" && event.gameId) {
    recordMediaTiming("IMG_DECODED", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
      durationMs: event.decodeTimeMs,
    });
  } else if (event.type === "error" && event.gameId) {
    recordMediaTiming("IMG_ERROR", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  } else if (event.type === "cache-hit" && event.gameId) {
    recordMediaTiming("BACKGROUND_CACHE_HIT", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  } else if (event.type === "cache-miss" && event.gameId) {
    recordMediaTiming("BACKGROUND_CACHE_MISS", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  } else if (event.type === "cache-evict" && event.gameId) {
    recordMediaTiming("BACKGROUND_CACHE_EVICT", {
      gameId: event.gameId,
      type: "hero",
      path: event.url,
    });
  }
  const debug = useNavigationStore.getState().debug;
  const patch = {
    backgroundRequestId:
      "requestId" in event ? event.requestId : debug.backgroundRequestId,
    backgroundStatus: event.type,
    backgroundPending:
      event.type === "request" || event.type === "pending"
        ? event.type === "request" || event.pending
        : event.type === "decoded" ||
            event.type === "error" ||
            event.type === "crossfade-finished"
          ? false
          : debug.backgroundPending,
    backgroundDecodeTimeMs:
      event.type === "decoded"
        ? event.decodeTimeMs
        : debug.backgroundDecodeTimeMs,
    backgroundCacheHits:
      event.type === "cache-hit"
        ? (debug.backgroundCacheHits ?? 0) + 1
        : debug.backgroundCacheHits,
    backgroundCacheMisses:
      event.type === "cache-miss"
        ? (debug.backgroundCacheMisses ?? 0) + 1
        : debug.backgroundCacheMisses,
  };
  useNavigationStore.getState().updateDebug(patch);
}
