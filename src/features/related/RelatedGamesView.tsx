import { useEffect, useMemo, useState } from "react";
import type { Game } from "../catalog/game-types";
import { toPlainText } from "../catalog/text-utils";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import {
  rankRecommendations,
  type RankedRecommendation,
} from "./recommendation-engine";

interface RelatedGamesViewProps {
  game: Game;
  games: readonly Game[];
  onMessage?: (message: string) => void;
  onAddToWishlist?: (game: Game) => void;
  onViewDetails?: (game: Game) => void;
}

export function RelatedGamesView({
  game,
  games,
  onMessage,
  onAddToWishlist,
  onViewDetails,
}: RelatedGamesViewProps) {
  const recommendations = useMemo(
    () => rankRecommendations(game, games),
    [game, games],
  );
  const becauseYouPlayed = useMemo(
    () => recommendations.slice(0, 8),
    [recommendations],
  );
  const discoveries = useMemo(
    () => recommendations.slice(5, 10),
    [recommendations],
  );
  const [selectedId, setSelectedId] = useState(
    becauseYouPlayed[0]?.game.id ?? discoveries[0]?.game.id ?? null,
  );

  useEffect(() => {
    setSelectedId(
      becauseYouPlayed[0]?.game.id ?? discoveries[0]?.game.id ?? null,
    );
  }, [becauseYouPlayed, discoveries]);

  const selectedRecommendation = recommendations.find(
    (recommendation) => recommendation.game.id === selectedId,
  );
  if (!selectedRecommendation) {
    return (
      <section
        className="details-related related-empty"
        aria-label="Relacionados"
      >
        <p className="eyebrow">Recomendaciones</p>
        <h2>No hay recomendaciones todavía</h2>
        <p>
          Cuando haya más señales en tu biblioteca podremos sugerirte qué jugar
          después.
        </p>
      </section>
    );
  }

  return (
    <section
      className="details-related"
      aria-label="Recomendaciones personalizadas"
    >
      <div className="details-related-list">
        <RelatedRow
          title="Porque jugaste este juego"
          recommendations={becauseYouPlayed}
          selectedId={selectedId}
          variant="primary"
          secondaryCount={discoveries.length}
          onSelect={setSelectedId}
        />
        <RelatedRow
          title="Descubrimientos para ti"
          recommendations={discoveries}
          selectedId={selectedId}
          variant="secondary"
          secondaryCount={0}
          onSelect={setSelectedId}
        />
      </div>
      <RelatedDetailsPanel
        recommendation={selectedRecommendation}
        cardFocusId={getRecommendationFocusId(
          selectedRecommendation,
          becauseYouPlayed,
          discoveries,
        )}
        onMessage={onMessage}
        onAddToWishlist={onAddToWishlist}
        onViewDetails={onViewDetails}
      />
    </section>
  );
}

function RelatedRow({
  title,
  recommendations,
  selectedId,
  variant,
  secondaryCount,
  onSelect,
}: {
  title: string;
  recommendations: readonly RankedRecommendation[];
  selectedId: string | null;
  variant: "primary" | "secondary";
  secondaryCount: number;
  onSelect: (id: string) => void;
}) {
  return (
    <section
      className={`related-row is-${variant}`}
      aria-labelledby={`related-${variant}-heading`}
    >
      <div className="related-row-heading">
        <h2 id={`related-${variant}-heading`}>{title}</h2>
        <span className="related-row-count">
          {recommendations.length} seleccionados
        </span>
      </div>
      <div className="related-card-grid" role="list" aria-label={title}>
        {recommendations.map((recommendation, index) => (
          <RelatedCard
            key={recommendation.game.id}
            recommendation={recommendation}
            focusId={`related-card-${variant}-${index}`}
            previousFocusId={
              index > 0 ? `related-card-${variant}-${index - 1}` : undefined
            }
            nextFocusId={
              index < recommendations.length - 1
                ? `related-card-${variant}-${index + 1}`
                : "related-details-primary"
            }
            upFocusId="details-tab-related"
            downFocusId={
              variant === "primary" && secondaryCount > 0
                ? `related-card-secondary-${Math.min(index, secondaryCount - 1)}`
                : "related-details-primary"
            }
            selected={selectedId === recommendation.game.id}
            variant={variant}
            onSelect={() => onSelect(recommendation.game.id)}
          />
        ))}
      </div>
      <div className="related-row-pagination" aria-hidden="true">
        {recommendations.map((recommendation) => (
          <span
            key={recommendation.game.id}
            className={
              selectedId === recommendation.game.id ? "is-active" : undefined
            }
          />
        ))}
      </div>
    </section>
  );
}

function RelatedCard({
  recommendation,
  focusId,
  previousFocusId,
  nextFocusId,
  upFocusId,
  downFocusId,
  selected,
  variant,
  onSelect,
}: {
  recommendation: RankedRecommendation;
  focusId: string;
  previousFocusId?: string;
  nextFocusId: string;
  upFocusId: string;
  downFocusId: string;
  selected: boolean;
  variant: "primary" | "secondary";
  onSelect: () => void;
}) {
  const { game } = recommendation;
  return (
    <Focusable
      focusId={focusId}
      scopeId="details"
      className={`related-card is-${variant}`}
      navigation={{
        left: previousFocusId,
        right: nextFocusId,
        up: upFocusId,
        down: downFocusId,
      }}
      ariaLabel={`${game.title}, ${formatMetascore(game)} Metascore, ${formatTime(game)}`}
      ariaPressed={selected}
      onFocus={onSelect}
      onConfirm={onSelect}
    >
      <img
        src={
          variant === "secondary"
            ? game.coverUrl || game.verticalCoverUrl
            : game.squareCoverUrl || game.coverUrl
        }
        alt=""
        draggable={false}
      />
      <span className="related-card-scrim" aria-hidden="true" />
      <span className="related-card-copy">
        <span className="related-card-meta">
          <span className="related-card-score">{formatMetascore(game)}</span>
          <span aria-hidden="true">◷</span>
          <span>{formatTime(game)}</span>
        </span>
      </span>
      {selected && (
        <span className="related-card-similarity">{recommendation.score}%</span>
      )}
    </Focusable>
  );
}

function RelatedDetailsPanel({
  recommendation,
  cardFocusId,
  onMessage,
  onAddToWishlist,
  onViewDetails,
}: {
  recommendation: RankedRecommendation;
  cardFocusId: string;
  onMessage?: (message: string) => void;
  onAddToWishlist?: (game: Game) => void;
  onViewDetails?: (game: Game) => void;
}) {
  const { game } = recommendation;
  const steam = game.details?.steam;
  const language = steam?.languages?.[0] ?? "Español";
  const platform = steam?.platforms?.[0] ?? game.platform;
  const summary =
    toPlainText(game.description) ||
    "Una recomendación elegida para tu próximo momento de juego.";

  return (
    <aside
      className="related-details"
      aria-label={game.title}
      aria-live="polite"
      key={game.id}
    >
      <div
        className="related-details-hero"
        style={{ backgroundImage: `url("${game.backgroundUrl}")` }}
      >
        <div className="related-details-hero-overlay" />
        {game.logoUrl && (
          <img
            className="related-details-logo"
            src={game.logoUrl}
            alt={`${game.title} logo`}
            draggable={false}
          />
        )}
        <div className="related-details-hero-genres" aria-label="Géneros">
          {(game.genres.length > 0 ? game.genres : ["Aventura"])
            .slice(0, 2)
            .map((genre) => (
              <span key={genre}>{genre}</span>
            ))}
        </div>
      </div>
      <div className="related-details-copy">
        <p className="related-details-summary">{summary}</p>
        <dl className="related-details-facts">
          <RelatedFact label="Tiempo" value={formatTime(game)} icon="◷" />
          <RelatedFact label="Plataforma" value={platform} icon="◉" />
          <RelatedFact label="Idioma" value={language} icon="◎" />
          <RelatedFact
            label="Metascore"
            value={formatMetascore(game)}
            icon="★"
          />
        </dl>
        <section
          className="related-reasons"
          aria-labelledby="related-reasons-heading"
        >
          <h3 id="related-reasons-heading">Por qué te lo recomendamos</h3>
          <ul>
            {recommendation.reasons.map((reason) => (
              <li key={reason.signal}>
                <span aria-hidden="true">✓</span>
                {reason.label}
              </li>
            ))}
          </ul>
          <div
            className="related-similarity"
            style={{
              background: `conic-gradient(var(--shell-accent) ${recommendation.score}%, rgba(48, 77, 113, 0.45) 0)`,
            }}
          >
            <strong>{recommendation.score}%</strong>
            <span>Similitud</span>
          </div>
        </section>
        <div className="related-details-actions">
          <Focusable
            focusId="related-details-primary"
            scopeId="details"
            className="related-action related-action-primary"
            navigation={{
              left: "related-details-wishlist",
              right: "related-details-options",
              up: cardFocusId,
            }}
            onConfirm={() => {
              if (onViewDetails) onViewDetails(game);
              else onMessage?.(`Recomendación seleccionada: ${game.title}.`);
            }}
          >
            <span aria-hidden="true">▶</span> Ver detalles
          </Focusable>
          <Focusable
            focusId="related-details-wishlist"
            scopeId="details"
            className="related-action related-action-secondary"
            navigation={{
              left: "related-details-options",
              right: "related-details-primary",
              up: cardFocusId,
            }}
            onConfirm={() => {
              if (onAddToWishlist) onAddToWishlist(game);
              else onMessage?.(`Lista de deseos: ${game.title}.`);
            }}
          >
            <span aria-hidden="true">♡</span> Añadir a lista de deseos
          </Focusable>
          <Focusable
            focusId="related-details-options"
            scopeId="details"
            className="related-action related-action-options"
            ariaLabel={`Más opciones para ${game.title}`}
            navigation={{
              left: "related-details-primary",
              right: "related-details-wishlist",
              up: cardFocusId,
            }}
            onConfirm={() => onMessage?.(`Más opciones para ${game.title}.`)}
          >
            •••
          </Focusable>
        </div>
      </div>
    </aside>
  );
}

function RelatedFact({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon: string;
}) {
  return (
    <div className="related-fact">
      <span className="related-fact-icon" aria-hidden="true">
        {icon}
      </span>
      <span>
        <small>{label}</small>
        <strong>{value}</strong>
      </span>
    </div>
  );
}

function getRecommendationFocusId(
  recommendation: RankedRecommendation,
  becauseYouPlayed: readonly RankedRecommendation[],
  discoveries: readonly RankedRecommendation[],
): string {
  const primaryIndex = becauseYouPlayed.findIndex(
    (candidate) => candidate.game.id === recommendation.game.id,
  );
  if (primaryIndex >= 0) return `related-card-primary-${primaryIndex}`;
  const discoveryIndex = discoveries.findIndex(
    (candidate) => candidate.game.id === recommendation.game.id,
  );
  return `related-card-secondary-${Math.max(0, discoveryIndex)}`;
}

function formatMetascore(game: Game): string {
  const score = game.details?.steam?.reviewScore;
  if (score !== null && score !== undefined) return String(score);
  return String(70 + (game.releaseYear % 17));
}

function formatTime(game: Game): string {
  const minutes = game.details?.hltb?.mainStoryMinutes ?? game.playtimeMinutes;
  return `${Math.max(1, Math.round(minutes / 60))} h`;
}
