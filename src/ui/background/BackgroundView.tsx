import { useEffect, useMemo, useSyncExternalStore } from "react";
import type { Game } from "../../features/catalog/game-types";
import { useNavigationStore } from "../../stores/navigation-store";
import {
  BackgroundManager,
  type BackgroundTelemetry,
} from "./background-manager";

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
      games[index - 1]?.backgroundUrl,
      games[index + 1]?.backgroundUrl,
      games[index + 2]?.backgroundUrl,
    ];
  }, [games, targetGame]);

  useEffect(() => {
    manager.request(targetGame?.backgroundUrl ?? null, navigationPhase);
    manager.preload(preloadUrls);
  }, [manager, navigationPhase, preloadUrls, targetGame]);

  useEffect(() => () => manager.dispose(), [manager]);

  return (
    <div className="background-view" aria-hidden="true">
      <div
        className="background-layer background-layer-current"
        style={
          snapshot.currentUrl
            ? { backgroundImage: `url("${snapshot.currentUrl}")` }
            : undefined
        }
      />
      <div
        className={`background-layer background-layer-incoming${snapshot.incomingVisible ? " is-visible" : ""}`}
        style={
          snapshot.incomingUrl
            ? { backgroundImage: `url("${snapshot.incomingUrl}")` }
            : undefined
        }
      />
      <div className="background-vignette" />
    </div>
  );
}

function handleTelemetry(event: BackgroundTelemetry): void {
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
