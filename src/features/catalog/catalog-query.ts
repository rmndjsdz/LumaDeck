import { useQuery } from "@tanstack/react-query";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { createMockCatalog } from "./mock-catalog";
import type { Game } from "./game-types";
import { recordMediaTiming } from "../../ui/performance/media-timing";

export function useGames() {
  return useQuery({
    queryKey: ["games"],
    queryFn: async (): Promise<Game[]> => {
      if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
        return createMockCatalog();
      }
      try {
        const localGames = await invoke<Game[]>("get_library_games");
        const resolvedGames = await Promise.all(
          localGames.map((game) => resolveLocalGameAssets(game)),
        );
        for (const game of resolvedGames) {
          recordMediaTiming("REACT_DATA_READY", {
            gameId: game.id,
            type: "hero",
            path: game.backgroundUrl,
            detail: `screenshots=${game.screenshots.length}`,
          });
        }
        return resolvedGames;
      } catch {
        return createMockCatalog();
      }
    },
    staleTime: 30_000,
    gcTime: 300_000,
  });
}

export async function fetchGameDetails(game: Game): Promise<Game> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return game;
  }
  const hydratedGame = await invoke<Game>("get_library_game", {
    gameId: game.id,
  });
  return resolveLocalGameAssets(hydratedGame);
}

export async function resolveLocalGameAssets(game: Game): Promise<Game> {
  const mediaServerUrl = await getMediaServerUrl();
  return {
    ...game,
    coverUrl: resolveAssetUrl(game.id, "grid", game.coverUrl, mediaServerUrl),
    verticalCoverUrl: resolveAssetUrl(
      game.id,
      "grid",
      game.verticalCoverUrl,
      mediaServerUrl,
    ),
    squareCoverUrl: resolveAssetUrl(
      game.id,
      "grid",
      game.squareCoverUrl ?? "",
      mediaServerUrl,
    ),
    logoUrl: resolveAssetUrl(game.id, "logo", game.logoUrl, mediaServerUrl),
    backgroundUrl: resolveAssetUrl(
      game.id,
      "hero",
      game.backgroundUrl,
      mediaServerUrl,
    ),
    iconUrl: resolveAssetUrl(
      game.id,
      "grid",
      game.iconUrl ?? "",
      mediaServerUrl,
    ),
    screenshots: game.screenshots.map((value) =>
      resolveAssetUrl(game.id, "screenshot", value, mediaServerUrl),
    ),
  };
}

let mediaServerUrlPromise: Promise<string> | undefined;

function getMediaServerUrl(): Promise<string> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return Promise.resolve("");
  }
  mediaServerUrlPromise ??= invoke<string>("get_media_server_url").catch(
    () => "",
  );
  return mediaServerUrlPromise;
}

function resolveAssetUrl(
  gameId: string,
  type: "hero" | "grid" | "screenshot" | "logo",
  value: string,
  mediaServerUrl: string,
): string {
  if (
    !value ||
    /^(https?:|data:|blob:|asset:|http:\/\/asset\.localhost)/i.test(value)
  ) {
    return value;
  }
  const url = mediaServerUrl
    ? `${mediaServerUrl}/media?path=${encodeURIComponent(value)}`
    : convertFileSrc(value);
  recordMediaTiming("ASSET_URL_CREATED", {
    gameId,
    type,
    path: value,
    url,
  });
  return url;
}
