import type { Game } from "../catalog/game-types";
import { getVisibleGames } from "../catalog/game-visibility";

export function resolveFeaturedGame(
  games: Game[],
  activeFocusId: string | null,
  continuePlaying?: Game[],
  favorites?: Game[],
): Game | undefined {
  const visibleGames = getVisibleGames(games);
  const visibleContinuePlaying = continuePlaying
    ? getVisibleGames(continuePlaying)
    : visibleGames.filter((game) => game.status === "playing").slice(0, 5);
  const visibleFavorites = favorites
    ? getVisibleGames(favorites)
    : visibleGames.filter((game) => game.favorite).slice(0, 5);
  const rows = [
    ["home-continue", visibleContinuePlaying],
    ["home-recent", visibleContinuePlaying],
    ["home-favorite", visibleFavorites],
  ] as const;
  for (const [prefix, rowGames] of rows) {
    const focusedGame = rowGames.find(
      (game) => activeFocusId === `${prefix}-${game.id}`,
    );
    if (focusedGame) return focusedGame;
  }
  return visibleContinuePlaying[0] ?? visibleGames[0];
}
