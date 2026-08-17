import "../cinematic-home.css";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { MediaImage } from "../../../../ui/performance/MediaImage";
import { Focusable } from "../../../../ui/navigation/focus/Focusable";
import { NavigationRow } from "../../../../ui/navigation/layouts/NavigationRow";
import { NavigationRowGroup } from "../../../../ui/navigation/layouts/NavigationRowGroup";
import type { Game } from "../../../catalog/game-types";
import { getAgeRatingAsset } from "../age-rating-assets";
import type { HomePresentationModel } from "../home-presentation";
import achievementsIcon from "../../../../assets/features/logros.png";
import { fetchGameDetails } from "../../../catalog/catalog-query";

interface CinematicHomeLayoutProps {
  presentation: HomePresentationModel;
  onOpen: (game: Game) => void;
}

export function CinematicHomeLayout({
  presentation,
  onOpen,
}: CinematicHomeLayoutProps) {
  const summaryGame = presentation.focusedGame;
  if (!summaryGame) {
    return <p className="empty-state cinematic-home-empty">No games yet.</p>;
  }

  return (
    <CinematicHomeContent
      summaryGame={summaryGame}
      presentation={presentation}
      onOpen={onOpen}
    />
  );
}

function CinematicHomeContent({
  summaryGame,
  presentation,
  onOpen,
}: {
  summaryGame: Game;
  presentation: HomePresentationModel;
  onOpen: (game: Game) => void;
}) {
  const { data: detailedGame } = useQuery({
    queryKey: ["game-details", summaryGame.id],
    queryFn: () => fetchGameDetails(summaryGame),
    staleTime: Infinity,
  });
  const game = detailedGame?.id === summaryGame.id ? detailedGame : summaryGame;
  const cinematicLayers = useCinematicGameLayers(game);
  const focusSettled = useCinematicFocusSettled(game.id);
  const ageRating = resolveAgeRating(game);

  return (
    <section
      className={"cinematic-home" + (focusSettled ? " is-focus-settled" : "")}
      aria-labelledby="cinematic-home-heading"
    >
      <div className="cinematic-hero">
        <div className="cinematic-hero-art-stack" aria-hidden="true">
          <CinematicHeroLayer
            key={"cinematic-hero-layer-" + cinematicLayers.heroGame.id}
            game={cinematicLayers.heroGame}
            className={
              "cinematic-hero-art-layer" +
              (cinematicLayers.heroIncomingVisible ? " is-outgoing" : "")
            }
          />
          {cinematicLayers.heroIncomingGame && (
            <CinematicHeroLayer
              key={
                "cinematic-hero-layer-" + cinematicLayers.heroIncomingGame.id
              }
              game={cinematicLayers.heroIncomingGame}
              className={
                "cinematic-hero-art-layer is-incoming" +
                (cinematicLayers.heroIncomingVisible ? " is-visible" : "")
              }
            />
          )}
        </div>
        <div className="cinematic-hero-fade" aria-hidden="true" />
        <div className="cinematic-hero-logo">
          {cinematicLayers.logoGame && (
            <CinematicLogoLayer
              game={cinematicLayers.logoGame}
              className={
                "cinematic-logo-layer" +
                (cinematicLayers.logoVisible ? " is-visible" : " is-entering")
              }
            />
          )}
          <h1 id="cinematic-home-heading" className="visually-hidden">
            {game.title}
          </h1>
        </div>
        {ageRating && (
          <div
            key={game.id}
            className="cinematic-age-rating-overlay"
            aria-label={`Age rating ${ageRating}`}
          >
            <span aria-hidden="true">{formatAgeRatingBadge(ageRating)}</span>
          </div>
        )}
      </div>
      <CinematicMetadata game={game} />
      <NavigationRowGroup
        scopeId="product-shell"
        groupId="home-rows"
        orientation="vertical"
        preserveHorizontalIntent
        regionId="home-content"
        parentRegionId="main-navigation"
        entryFocusId={`home-cinematic-${presentation.railGames[0]?.id ?? "empty"}`}
        exitFocusId="main-nav-home"
        className="cinematic-rail-region"
      >
        <NavigationRow rowId="home-cinematic-rail" rowIndex={0}>
          {presentation.railGames.map((railGame, index) => (
            <Focusable
              key={railGame.id}
              focusId={`home-cinematic-${railGame.id}`}
              scopeId="product-shell"
              itemIndex={index}
              className="cinematic-rail-card"
              ariaLabel={railGame.title}
              onConfirm={() => onOpen(railGame)}
            >
              <div className="cinematic-rail-art-frame">
                <MediaImage
                  gameId={railGame.id}
                  mediaType="grid"
                  className="cinematic-rail-art"
                  src={
                    railGame.squareCoverUrl ||
                    railGame.verticalCoverUrl ||
                    railGame.coverUrl
                  }
                  alt=""
                  draggable={false}
                  loading={index < 3 ? "eager" : "lazy"}
                />
              </div>
              <span className="visually-hidden">{railGame.title}</span>
            </Focusable>
          ))}
        </NavigationRow>
      </NavigationRowGroup>
    </section>
  );
}

interface CinematicGameLayers {
  targetGameId: string;
  heroGame: Game;
  heroIncomingGame: Game | null;
  heroIncomingVisible: boolean;
  logoGame: Game | null;
  logoVisible: boolean;
}

function useCinematicGameLayers(game: Game): CinematicGameLayers {
  const [layers, setLayers] = useState<CinematicGameLayers>(() => ({
    targetGameId: game.id,
    heroGame: game,
    heroIncomingGame: null,
    heroIncomingVisible: false,
    logoGame: game,
    logoVisible: true,
  }));
  const previousGameIdRef = useRef(game.id);
  const transitionTokenRef = useRef(0);

  const renderedLayers =
    layers.targetGameId === game.id
      ? layers
      : {
          ...layers,
          targetGameId: game.id,
          heroGame:
            layers.heroIncomingVisible && layers.heroIncomingGame
              ? layers.heroIncomingGame
              : layers.heroGame,
          heroIncomingGame: game,
          heroIncomingVisible: false,
          logoGame: null,
          logoVisible: false,
        };

  useLayoutEffect(() => {
    if (previousGameIdRef.current === game.id) return;
    previousGameIdRef.current = game.id;
    const transitionToken = transitionTokenRef.current + 1;
    transitionTokenRef.current = transitionToken;

    setLayers((current) => ({
      targetGameId: game.id,
      heroGame:
        current.heroIncomingVisible && current.heroIncomingGame
          ? current.heroIncomingGame
          : current.heroGame,
      heroIncomingGame: game,
      heroIncomingVisible: false,
      logoGame: null,
      logoVisible: false,
    }));

    const logoEnterTimer = window.setTimeout(() => {
      if (transitionTokenRef.current !== transitionToken) return;
      setLayers((current) =>
        current.targetGameId === game.id
          ? { ...current, logoGame: game, logoVisible: false }
          : current,
      );
    }, CINEMATIC_LOGO_DELAY_MS);
    const logoVisibleTimer = window.setTimeout(() => {
      if (transitionTokenRef.current !== transitionToken) return;
      setLayers((current) =>
        current.targetGameId === game.id && current.logoGame?.id === game.id
          ? { ...current, logoVisible: true }
          : current,
      );
    }, CINEMATIC_LOGO_DELAY_MS + 1);
    const heroVisibleTimer = window.setTimeout(() => {
      if (transitionTokenRef.current !== transitionToken) return;
      setLayers((current) =>
        current.targetGameId === game.id
          ? { ...current, heroIncomingVisible: true }
          : current,
      );
    }, CINEMATIC_HERO_DELAY_MS);
    const settleTimer = window.setTimeout(() => {
      if (transitionTokenRef.current !== transitionToken) return;
      setLayers((current) =>
        current.targetGameId === game.id
          ? {
              targetGameId: game.id,
              heroGame: game,
              heroIncomingGame: null,
              heroIncomingVisible: false,
              logoGame: game,
              logoVisible: true,
            }
          : current,
      );
    }, CINEMATIC_TRANSITION_SETTLE_MS);

    return () => {
      window.clearTimeout(logoEnterTimer);
      window.clearTimeout(logoVisibleTimer);
      window.clearTimeout(heroVisibleTimer);
      window.clearTimeout(settleTimer);
    };
  }, [game]);

  return renderedLayers;
}

function useCinematicFocusSettled(focusId: string): boolean {
  const focusTokenRef = useRef({ focusId, token: 0 });
  if (focusTokenRef.current.focusId !== focusId) {
    focusTokenRef.current = {
      focusId,
      token: focusTokenRef.current.token + 1,
    };
  }
  const focusToken = focusTokenRef.current.token;
  const [settledToken, setSettledToken] = useState<number | null>(null);

  useEffect(() => {
    const settleTimer = window.setTimeout(() => {
      setSettledToken(focusToken);
    }, CINEMATIC_FOCUS_IDLE_DELAY_MS);

    return () => window.clearTimeout(settleTimer);
  }, [focusId, focusToken]);

  return settledToken === focusToken;
}

const CINEMATIC_FOCUS_IDLE_DELAY_MS = 380;
const CINEMATIC_LOGO_DELAY_MS = 650;
const CINEMATIC_HERO_DELAY_MS = 750;
const CINEMATIC_TRANSITION_SETTLE_MS = 1_300;

function CinematicHeroLayer({
  game,
  className,
}: {
  game: Game;
  className: string;
}) {
  return (
    <MediaImage
      gameId={game.id}
      mediaType="hero"
      className={`cinematic-hero-art ${className}`}
      src={game.backgroundUrl}
      alt=""
      aria-hidden="true"
      loading="eager"
      decoding="async"
      reactKey={`cinematic-hero-${game.id}`}
    />
  );
}

function CinematicLogoLayer({
  game,
  className,
}: {
  game: Game;
  className: string;
}) {
  return (
    <div className={className} aria-hidden="true">
      {game.logoUrl.trim() ? (
        <MediaImage
          gameId={game.id}
          mediaType="logo"
          className="cinematic-game-logo"
          src={game.logoUrl}
          alt=""
          loading="eager"
          decoding="async"
          reactKey={`cinematic-logo-${game.id}`}
        />
      ) : (
        <span className="cinematic-game-title">{game.title}</span>
      )}
    </div>
  );
}

type CinematicMetadataItem = {
  value: string;
  image?: string;
  iconImage?: string;
  icon?: CinematicMetadataIconName;
};

type CinematicMetadataIconName =
  "calendar" | "history" | "platform" | "score" | "storage" | "tag";

function CinematicMetadata({ game }: { game: Game }) {
  const achievementLabel = formatAchievements(game.achievements);
  const reviewScore = game.details?.steam?.reviewScore;
  const ageRating = resolveAgeRating(game);
  const ageRatingAsset = getAgeRatingAsset(ageRating);
  const genreLabel = game.genres.slice(0, 2).join(" ");
  const releaseLabel = formatReleaseDate(game);
  const items: Array<CinematicMetadataItem | null> = [
    genreLabel ? { icon: "tag", value: genreLabel } : null,
    releaseLabel ? { icon: "calendar", value: releaseLabel } : null,
    reviewScore !== null && reviewScore !== undefined
      ? { icon: "score", value: reviewScore.toFixed(1) }
      : null,
    achievementLabel
      ? { iconImage: achievementsIcon, value: achievementLabel }
      : null,
    { icon: "history", value: formatLastPlayed(game.lastPlayedAt) },
    game.installSizeGb !== undefined
      ? {
          icon: "storage",
          value: formatInstallSize(game.installSizeGb),
        }
      : null,
    ageRatingAsset && ageRating
      ? {
          value: `PEGI ${ageRating}`,
          image: ageRatingAsset,
        }
      : null,
    game.provider || game.platform
      ? { icon: "platform", value: game.provider || game.platform }
      : null,
  ];
  const visibleItems = items.filter(
    (item): item is CinematicMetadataItem => item !== null,
  );

  return (
    <div className="cinematic-metadata" aria-label={`${game.title} metadata`}>
      {visibleItems.map((item, index) => (
        <span
          className="cinematic-metadata-item"
          key={`${index}-${item.value}`}
        >
          {item.image ? (
            <img
              className="cinematic-age-rating"
              src={item.image}
              alt={item.value}
            />
          ) : item.iconImage ? (
            <img
              className="cinematic-metadata-icon"
              src={item.iconImage}
              alt=""
              aria-hidden="true"
            />
          ) : (
            item.icon && <CinematicMetadataIcon name={item.icon} />
          )}
          {!item.image && item.value}
        </span>
      ))}
    </div>
  );
}

function CinematicMetadataIcon({ name }: { name: CinematicMetadataIconName }) {
  const paths: Record<CinematicMetadataIconName, React.ReactNode> = {
    tag: <path d="m3 12 9 9 9-9V4H13L3 12Zm11-4h.01" />,
    calendar: (
      <>
        <path d="M6 2v4M18 2v4M3 9h18" />
        <rect x="3" y="4" width="18" height="17" rx="2" />
        <path d="M8 13h3v3H8zM14 13h3v3h-3z" />
      </>
    ),
    score: (
      <path d="m12 2 3.1 6.3 6.9 1-5 4.9 1.2 6.8-6.2-3.2L5.8 21 7 14.2l-5-4.9 6.9-1L12 2Z" />
    ),
    history: (
      <>
        <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
        <path d="M3 3v5h5M12 7v5l3 2" />
      </>
    ),
    storage: (
      <>
        <path d="M5 5h14l3 7H2l3-7Z" />
        <path d="M2 12v7h20v-7M17 16h.01" />
      </>
    ),
    platform: (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" />
      </>
    ),
  };
  return (
    <svg
      className="cinematic-metadata-symbol"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}

function resolveAgeRating(game: Game): string | undefined {
  return game.ageRating
    ?.replace(/^PEGI\s*/i, "")
    .replace(/\+$/, "")
    .trim();
}

function formatAgeRatingBadge(ageRating: string): string {
  return /^\d+$/.test(ageRating) ? `${ageRating}+` : ageRating;
}

function formatAchievements(achievements: Game["achievements"]): string | null {
  if (!achievements) return null;
  const unlocked = achievements.unlocked ?? "—";
  const total = achievements.total ?? "—";
  const progress =
    achievements.progress ??
    (typeof achievements.unlocked === "number" &&
    typeof achievements.total === "number" &&
    achievements.total > 0
      ? (achievements.unlocked / achievements.total) * 100
      : null);
  return progress === null
    ? `${unlocked} / ${total}`
    : `${unlocked} / ${total} (${Math.round(progress)}%)`;
}

function formatReleaseDate(game: Game): string | null {
  const rawDate =
    game.details?.steam?.releaseDate ?? game.details?.launchbox?.releaseDate;
  if (rawDate) return formatDate(rawDate);
  return game.releaseYear > 0 ? String(game.releaseYear) : null;
}

function formatLastPlayed(value: string | null): string {
  return value ? formatDate(value) : "Not Played";
}

function formatDate(value: string): string {
  const numericValue = Number(value);
  const date = new Date(
    Number.isFinite(numericValue) && numericValue > 0
      ? numericValue < 100_000_000_000
        ? numericValue * 1000
        : numericValue
      : value,
  );
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("en-GB", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  }).format(date);
}

function formatInstallSize(sizeGb: number): string {
  return `${Number.isInteger(sizeGb) ? sizeGb : sizeGb.toFixed(1)} GB`;
}
