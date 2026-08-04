import { useQuery } from "@tanstack/react-query";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { createMockCatalog } from "./mock-catalog";
import type { Game } from "./game-types";

export function useGames() {
  return useQuery({
    queryKey: ["games"],
    queryFn: async (): Promise<Game[]> => {
      if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
        return createMockCatalog();
      }
      try {
        const localGames = await invoke<Game[]>("get_library_games");
        return localGames.map(resolveLocalGameAssets);
      } catch {
        return createMockCatalog();
      }
    },
    staleTime: 30_000,
    gcTime: 300_000,
  });
}

function resolveLocalGameAssets(game: Game): Game {
  return {
    ...game,
    coverUrl: resolveAssetUrl(game.coverUrl),
    verticalCoverUrl: resolveAssetUrl(game.verticalCoverUrl),
    logoUrl: resolveAssetUrl(game.logoUrl),
    backgroundUrl: resolveAssetUrl(game.backgroundUrl),
    screenshots: game.screenshots.map(resolveAssetUrl),
  };
}

function resolveAssetUrl(value: string): string {
  if (
    !value ||
    /^(https?:|data:|blob:|asset:|http:\/\/asset\.localhost)/i.test(value)
  ) {
    return value;
  }
  return convertFileSrc(value);
}
