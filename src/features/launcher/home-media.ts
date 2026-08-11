import type { Game } from "../catalog/game-types";
import { getVisibleGames } from "../catalog/game-visibility";
import { resolveFeaturedGame } from "./home-feature-selection";

export function getHomeCriticalGames(games: readonly Game[]): Game[] {
  const visibleGames = getVisibleGames(games);
  const playingGames = visibleGames.filter((game) => game.status === "playing");
  const continuePlaying = (playingGames.length ? playingGames : visibleGames)
    .filter((game) => game.lastPlayedAt && game.playtimeMinutes > 0)
    .sort((left, right) =>
      (right.lastPlayedAt ?? "").localeCompare(left.lastPlayedAt ?? ""),
    )
    .slice(0, 5);
  const savedFavorites = visibleGames.filter((game) => game.favorite);
  const favorites = (savedFavorites.length ? savedFavorites : visibleGames)
    .filter((game) => game.coverUrl || game.verticalCoverUrl)
    .slice(0, 6);
  const featuredGame = resolveFeaturedGame(
    visibleGames,
    null,
    continuePlaying,
    favorites,
  );
  const unique = new Map<string, Game>();
  for (const game of [featuredGame, ...continuePlaying, ...favorites]) {
    if (game) unique.set(game.id, game);
  }
  return [...unique.values()];
}
