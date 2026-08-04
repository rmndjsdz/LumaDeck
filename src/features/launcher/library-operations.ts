import type { Game, GameStatus } from "../catalog/game-types";
import type { LibrarySort } from "../../stores/library-store";

export type { LibrarySort } from "../../stores/library-store";

export function filterAndSortGames(
  games: readonly Game[],
  query: string,
  status: "all" | GameStatus,
  sort: LibrarySort,
): Game[] {
  const normalized = query.trim().toLocaleLowerCase();
  return [...games]
    .filter(
      (game) =>
        !normalized || game.title.toLocaleLowerCase().includes(normalized),
    )
    .filter((game) => status === "all" || game.status === status)
    .sort((left, right) => {
      if (sort === "recent")
        return (right.lastPlayedAt ?? "").localeCompare(
          left.lastPlayedAt ?? "",
        );
      if (sort === "time") return right.playtimeMinutes - left.playtimeMinutes;
      return left.sortTitle.localeCompare(right.sortTitle);
    });
}
