import type { Game } from "../catalog/game-types";
import { buildHomePresentation } from "./home/home-presentation";

export function getHomeCriticalGames(games: readonly Game[]): Game[] {
  const { featuredGame, continuePlaying, favorites } = buildHomePresentation(
    games,
    null,
  );
  const unique = new Map<string, Game>();
  for (const game of [
    featuredGame,
    ...continuePlaying.slice(0, 2),
    ...favorites.slice(0, 1),
  ]) {
    if (game) unique.set(game.id, game);
  }
  return [...unique.values()];
}
