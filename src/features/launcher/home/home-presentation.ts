import type { Game } from "../../catalog/game-types";
import { getVisibleGames } from "../../catalog/game-visibility";
import { resolveFeaturedGame } from "../home-feature-selection";

export interface HomePresentationModel {
  visibleGames: Game[];
  continuePlaying: Game[];
  favorites: Game[];
  railGames: Game[];
  featuredGame: Game | undefined;
  focusedGame: Game | undefined;
}

export function buildHomePresentation(
  games: readonly Game[],
  activeFocusId: string | null,
): HomePresentationModel {
  const visibleGames = getVisibleGames(games);
  const playingGames = visibleGames.filter((game) => game.status === "playing");
  const continuePlaying = (playingGames.length ? playingGames : visibleGames)
    .filter((game) => game.lastPlayedAt && game.playtimeMinutes > 0)
    .slice(0, 5);
  const savedFavorites = visibleGames.filter((game) => game.favorite);
  const favorites = (savedFavorites.length ? savedFavorites : visibleGames)
    .filter((game) => game.coverUrl || game.verticalCoverUrl)
    .slice(0, 6);
  const featuredGame = resolveFeaturedGame(
    visibleGames,
    activeFocusId,
    continuePlaying,
    favorites,
  );
  const uniqueRailGames = new Map<string, Game>();
  for (const game of [...continuePlaying, ...favorites, ...visibleGames]) {
    uniqueRailGames.set(game.id, game);
    if (uniqueRailGames.size === 20) break;
  }
  const railGames = [...uniqueRailGames.values()];
  const focusedGame =
    railGames.find((game) => activeFocusId === `home-cinematic-${game.id}`) ??
    featuredGame;

  return {
    visibleGames,
    continuePlaying,
    favorites,
    railGames,
    featuredGame,
    focusedGame,
  };
}
