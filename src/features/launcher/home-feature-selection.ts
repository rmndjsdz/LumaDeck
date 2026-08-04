import type { Game } from "../catalog/game-types";

export function resolveFeaturedGame(
  games: Game[],
  activeFocusId: string | null,
  continuePlaying: Game[] = games
    .filter((game) => game.status === "playing")
    .slice(0, 5),
  favorites: Game[] = games.filter((game) => game.favorite).slice(0, 5),
): Game | undefined {
  const rows = [
    ["home-continue", continuePlaying],
    ["home-recent", continuePlaying],
    ["home-favorite", favorites],
  ] as const;
  for (const [prefix, rowGames] of rows) {
    const focusedGame = rowGames.find(
      (game) => activeFocusId === `${prefix}-${game.id}`,
    );
    if (focusedGame) return focusedGame;
  }
  return continuePlaying[0] ?? games[0];
}
