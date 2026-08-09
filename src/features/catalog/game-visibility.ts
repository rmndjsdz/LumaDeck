import type { Game } from "./game-types";

export function isGameHidden(game: Pick<Game, "hidden">): boolean {
  return game.hidden === true;
}

export function getVisibleGames(games: readonly Game[]): Game[] {
  return games.filter((game) => !isGameHidden(game));
}
