import type { Game } from "../catalog/game-types";
import { getVisibleGames } from "../catalog/game-visibility";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { useNavigationStore } from "../../stores/navigation-store";
import { NavigationRow } from "../../ui/navigation/layouts/NavigationRow";
import { NavigationRowGroup } from "../../ui/navigation/layouts/NavigationRowGroup";
import type { NavigationScreenDefinition } from "../../ui/navigation/screen/navigation-screen-contract";
import { GameCard } from "./GameCard";
import { resolveFeaturedGame } from "./home-feature-selection";
import { MediaImage } from "../../ui/performance/MediaImage";

export const HOME_SCREEN_DEFINITION = {
  id: "home",
  route: "home",
  rootScope: { scopeId: "product-shell" },
  initialFocus: "main-nav-home",
  regions: [
    {
      regionId: "home-content",
      parentRegionId: "main-navigation",
      entryFocusId: "main-nav-home",
      exitFocusId: "main-nav-home",
    },
  ],
  rowGroups: [
    {
      groupId: "home-rows",
      orientation: "vertical",
      preserveHorizontalIntent: true,
    },
  ],
  restorePolicy: { restoreFocus: true, rememberScroll: true },
} satisfies NavigationScreenDefinition;

interface HomeViewProps {
  games: Game[];
  onOpen: (game: Game) => void;
  onViewLibrary: () => void;
}

export function HomeView({ games, onOpen, onViewLibrary }: HomeViewProps) {
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
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
    activeFocusId,
    continuePlaying,
    favorites,
  );
  const homeRegion = HOME_SCREEN_DEFINITION.regions[0];
  const homeRows = HOME_SCREEN_DEFINITION.rowGroups[0];

  if (!homeRegion || !homeRows) {
    throw new Error("Home navigation definition is incomplete");
  }

  return (
    <section className="product-page home-view" aria-labelledby="home-heading">
      {featuredGame && <HomeHero game={featuredGame} onOpen={onOpen} />}
      <NavigationRowGroup
        scopeId="product-shell"
        groupId={homeRows.groupId}
        orientation={homeRows.orientation}
        preserveHorizontalIntent={homeRows.preserveHorizontalIntent}
        regionId={homeRegion.regionId}
        parentRegionId={homeRegion.parentRegionId}
        entryFocusId={homeRegion.entryFocusId}
        exitFocusId={homeRegion.exitFocusId}
      >
        <GameRow
          title="Continue Playing"
          games={continuePlaying}
          prefix="home-continue"
          rowIndex={0}
          onOpen={onOpen}
          actionLabel="Ver todos"
          onAction={onViewLibrary}
        />
        <GameRow
          title="Favorites"
          games={favorites}
          prefix="home-favorite"
          rowIndex={1}
          onOpen={onOpen}
          actionLabel="Manage"
          actionIcon={"\u2661"}
          onAction={onViewLibrary}
        />
      </NavigationRowGroup>
    </section>
  );
}


function HomeHero({
  game,
  onOpen,
}: {
  game: Game;
  onOpen: (game: Game) => void;
}) {
  return (
    <section className="home-hero" aria-label={`Featured game: ${game.title}`}>
      <div className="home-hero-art-stack" aria-hidden="true">
        <div
          className="home-hero-art"
          style={{ backgroundImage: `url("${game.backgroundUrl}")` }}
        />
      </div>
      <div className="home-hero-scene home-hero-scene-current">
        {renderHeroScene(game, onOpen, "home-heading")}
      </div>
      <div className="home-hero-pagination" aria-label="Featured game 1 of 8">
        {Array.from({ length: 8 }, (_, index) => (
          <span className={index === 0 ? "is-active" : ""} key={index} />
        ))}
      </div>
    </section>
  );
}

function renderHeroScene(
  game: Game,
  onOpen: (game: Game) => void,
  headingId?: string,
) {
  return (
    <>
      <div className="home-hero-copy">
        <div className="home-hero-logo-slot">
          {game.logoUrl.trim() ? (
            <>
              <h1 id={headingId} className="visually-hidden">
                {game.title}
              </h1>
              <MediaImage
                gameId={game.id}
                mediaType="logo"
                className="home-hero-logo"
                src={game.logoUrl}
                alt={`${game.title} logo`}
                draggable={false}
                loading="eager"
                decoding="async"
              />
            </>
          ) : (
            <h1
              id={headingId}
              className="home-featured-title home-featured-title-fallback"
            >
              {game.title}
            </h1>
          )}
        </div>
        <div className="home-hero-details">
          <span>
            <span className="home-detail-icon" aria-hidden="true">
              {"\u25f7"}
            </span>
            Last played: {formatLastPlayed(game.lastPlayedAt)}
          </span>
          <span>
            <span className="home-detail-icon" aria-hidden="true">
              {"\u25f7"}
            </span>
            {formatPlaytime(game.playtimeMinutes)} played
          </span>
        </div>
        <div className="home-hero-actions">
          <Focusable
            focusId="home-hero-continue"
            scopeId="product-shell"
            className="home-primary-action"
            navigation={{ right: "home-hero-details" }}
            ariaLabel="Continue"
            onConfirm={() => onOpen(game)}
          >
            <span className="play-icon" aria-hidden="true">
              {"\u25b6"}
            </span>
            <span className="home-action-copy">
              <strong>Continue</strong>
              <small>Last played: {formatLastPlayed(game.lastPlayedAt)}</small>
            </span>
          </Focusable>
          <Focusable
            focusId="home-hero-details"
            scopeId="product-shell"
            className="home-secondary-action"
            navigation={{ left: "home-hero-continue" }}
            ariaLabel="Game Details"
            onConfirm={() => onOpen(game)}
          >
            <span className="home-details-icon" aria-hidden="true">
              {"\u229e"}
            </span>
            <span className="home-action-copy">
              <strong>Game Details</strong>
              <small>View game information</small>
            </span>
          </Focusable>
        </div>
      </div>
      <div className="home-hero-aside" aria-label="Game summary">
        <div className="home-stat-card">
          <div className="home-stat-row">
            <span className="home-stat-icon" aria-hidden="true">
              {"\u25f7"}
            </span>
            <span>Last played</span>
            <strong>{formatLastPlayed(game.lastPlayedAt)}</strong>
          </div>
          <div className="home-stat-row">
            <span className="home-stat-icon" aria-hidden="true">
              {"\u25f7"}
            </span>
            <span>Play time</span>
            <strong>{formatPlaytime(game.playtimeMinutes)}</strong>
          </div>
          <div className="home-stat-row">
            <span
              className="home-stat-icon home-stat-icon-trophy"
              aria-hidden="true"
            >
              {"\u265c"}
            </span>
            <span>Achievements</span>
            <strong>{formatAchievements(game)}</strong>
          </div>
        </div>
      </div>
    </>
  );
}

function formatLastPlayed(value: string | null): string {
  if (!value) return "Never";
  const date = parseSteamDate(value);
  if (Number.isNaN(date.getTime())) return value;
  const elapsedDays = Math.floor(
    (Date.now() - date.getTime()) / (1000 * 60 * 60 * 24),
  );
  if (elapsedDays <= 1) return "Today";
  if (elapsedDays < 7) return `${elapsedDays}d ago`;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function parseSteamDate(value: string): Date {
  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return new Date(
      numericValue < 100_000_000_000 ? numericValue * 1000 : numericValue,
    );
  }
  return new Date(value);
}

function formatPlaytime(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return hours > 0 ? `${hours}h ${remainingMinutes}m` : `${minutes}m`;
}

function formatAchievements(game: Game): string {
  const achievements = game.achievements;
  if (achievements?.unlocked != null && achievements.total != null) {
    return `${achievements.unlocked} / ${achievements.total}`;
  }
  return "\u2014 / \u2014";
}

function GameRow({
  title,
  games,
  prefix,
  rowIndex,
  onOpen,
  actionLabel,
  actionIcon,
  onAction,
}: {
  title: string;
  games: Game[];
  prefix: string;
  rowIndex: number;
  onOpen: (game: Game) => void;
  actionLabel: string;
  actionIcon?: string;
  onAction: () => void;
}) {
  return (
    <section
      className={`game-row ${prefix}`}
      aria-labelledby={`${prefix}-heading`}
    >
      <div className="row-heading">
        <h2 id={`${prefix}-heading`}>
          {prefix === "home-favorite" && (
            <span className="favorite-heading-icon" aria-hidden="true">
              {"\u2661"}
            </span>
          )}
          {title}
        </h2>
        <span>{games.length} shown</span>
      </div>
      <NavigationRow rowId={prefix} rowIndex={rowIndex}>
        {games.map((game, itemIndex) => (
          <GameCard
            key={game.id}
            game={game}
            focusId={`${prefix}-${game.id}`}
            itemIndex={itemIndex}
            onOpen={onOpen}
            compact
            vertical={prefix === "home-favorite"}
          />
        ))}
        <Focusable
          focusId={`${prefix}-action`}
          scopeId="product-shell"
          itemIndex={games.length}
          className="game-row-action"
          aria-label={actionLabel}
          onConfirm={onAction}
        >
          <span className="game-row-action-icon" aria-hidden="true">
            {actionIcon ?? "+"}
          </span>
          <span>{actionLabel}</span>
        </Focusable>
      </NavigationRow>
    </section>
  );
}
