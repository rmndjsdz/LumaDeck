import { openUrl } from "@tauri-apps/plugin-opener";
import { useState, type CSSProperties } from "react";
import { useQueryClient } from "@tanstack/react-query";
import metacriticLogo from "../../assets/metacritic-logo.png";
import opencriticLogo from "../../assets/opencritic-logo.png";
import steamLogo from "../../assets/steam-logo.png";
import "./reviews.css";
import { useGameReviewsSummary } from "./reviews-query";
import {
  reviewConsensusQueryKey,
  useGameReviewConsensus,
} from "./consensus-query";
import { consensusService } from "./consensus-service";
import { ConsensusPanel } from "./ConsensusPanel";
import { useProductStore } from "../../stores/product-store";
import type {
  GameReviewConsensus,
  ReviewConsensusQueryData,
} from "./consensus-types";
import {
  getCriticScoreClassification,
  getSteamScoreClassification,
} from "./reviews-score-classification";
import type {
  GameReviewsSummary,
  ReviewDistribution,
  ReviewProvider,
  ReviewSourceSummary,
  ReviewsGame,
} from "./reviews-types";

const SOURCE_META: Record<
  ReviewProvider,
  { name: string; logo: string; scoreLabel: string; countLabel: string }
> = {
  metacritic: {
    name: "Metacritic",
    logo: metacriticLogo,
    scoreLabel: "Puntuación de críticos",
    countLabel: "críticas",
  },
  opencritic: {
    name: "OpenCritic",
    logo: opencriticLogo,
    scoreLabel: "Puntuación de críticos",
    countLabel: "críticas",
  },
  steam: {
    name: "Steam",
    logo: steamLogo,
    scoreLabel: "Puntuación de usuarios",
    countLabel: "reseñas",
  },
};

export function ReviewsView({ game }: { game: ReviewsGame }) {
  const query = useGameReviewsSummary(game);
  const consensusQuery = useGameReviewConsensus(game.id);
  const queryClient = useQueryClient();
  const setView = useProductStore((state) => state.setView);
  const [generating, setGenerating] = useState(false);
  const [consensusError, setConsensusError] = useState<string | null>(null);

  const generateConsensus = async (forceRefresh: boolean) => {
    if (generating) return;
    setGenerating(true);
    setConsensusError(null);
    try {
      const consensus = await consensusService.generate(game.id, forceRefresh);
      queryClient.setQueryData<ReviewConsensusQueryData>(
        reviewConsensusQueryKey(game.id),
        (current) => ({
          consensus,
          aiConfigured: current?.aiConfigured ?? true,
        }),
      );
    } catch (error) {
      setConsensusError(
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "CONSENSUS_GENERATION_ERROR",
      );
    } finally {
      setGenerating(false);
    }
  };

  if (query.isPending) return <ReviewLoadingState />;
  if (query.isError || !query.data) {
    return <ReviewErrorState onRetry={() => void query.refetch()} />;
  }

  return (
    <ReviewsContent
      summary={query.data}
      consensus={consensusQuery.data?.consensus ?? null}
      aiConfigured={consensusQuery.data?.aiConfigured ?? false}
      consensusLoading={consensusQuery.isPending}
      generating={generating}
      consensusError={consensusError}
      onGenerate={() => void generateConsensus(false)}
      onUpdate={() => void generateConsensus(true)}
      onOpenSettings={() => setView("settings")}
    />
  );
}

function ReviewsContent({
  summary,
  consensus,
  aiConfigured,
  consensusLoading,
  generating,
  consensusError,
  onGenerate,
  onUpdate,
  onOpenSettings,
}: {
  summary: GameReviewsSummary;
  consensus: GameReviewConsensus | null;
  aiConfigured: boolean;
  consensusLoading: boolean;
  generating: boolean;
  consensusError: string | null;
  onGenerate: () => void;
  onUpdate: () => void;
  onOpenSettings: () => void;
}) {
  const partial = summary.status === "partial";

  return (
    <section className="reviews-view" aria-labelledby="reviews-heading">
      <div className="reviews-grid">
        <aside className="reviews-column reviews-sources-column">
          <div className="reviews-column-heading">
            <p className="reviews-eyebrow">Fuentes</p>
            <h2 id="reviews-heading">Puntuaciones de críticos</h2>
          </div>
          <div className="reviews-source-list">
            {(["metacritic", "opencritic", "steam"] as const).map(
              (provider) => (
                <ReviewSourceCard
                  key={provider}
                  source={summary.sources[provider]}
                />
              ),
            )}
          </div>
          {partial && (
            <p className="reviews-partial-note">
              Algunas fuentes no están disponibles. Se muestran los datos
              recibidos.
            </p>
          )}
        </aside>

        <ConsensusPanel
          summary={summary}
          consensus={consensus}
          aiConfigured={aiConfigured}
          loading={consensusLoading}
          generating={generating}
          errorMessage={consensusError}
          onGenerate={onGenerate}
          onUpdate={onUpdate}
          onOpenSettings={onOpenSettings}
        />

        <section className="reviews-column reviews-featured-column">
          <div className="reviews-column-heading reviews-featured-heading">
            <div>
              <p className="reviews-eyebrow">Steam</p>
              <h2>Reseñas destacadas</h2>
            </div>
            <span className="reviews-count-badge">
              {summary.featuredReviews.length} destacadas
            </span>
          </div>
          {summary.featuredReviews.length > 0 ? (
            <div className="reviews-featured-list">
              {summary.featuredReviews.map((review) => (
                <SteamReviewCard key={review.id} review={review} />
              ))}
            </div>
          ) : (
            <ReviewEmptyState
              title="Sin reseñas destacadas"
              message="Steam no ha devuelto reseñas visibles para este juego."
            />
          )}
          <SteamReviewsLink steamAppId={summary.steamAppId} />
        </section>

        <SteamSummaryCard summary={summary} />
      </div>
    </section>
  );
}

function ReviewSourceCard({ source }: { source: ReviewSourceSummary }) {
  const meta = SOURCE_META[source.provider];
  const hasScore = source.score !== null;
  const classification =
    source.provider === "steam"
      ? getSteamScoreClassification(source.score)
      : getCriticScoreClassification(source.provider, source.score);
  const status = source.error
    ? "Proveedor no disponible"
    : hasScore
      ? source.platform
        ? `Puntuación ${source.platform}`
        : meta.scoreLabel
      : "Sin datos disponibles";

  return (
    <article className={`reviews-source-card is-${source.status}`}>
      <div className={`reviews-source-mark is-${source.provider}`}>
        <img src={meta.logo} alt="" />
      </div>
      <div className="reviews-source-copy">
        <div className="reviews-source-name-row">
          <h3>{meta.name}</h3>
          <span className="reviews-source-status">{status}</span>
        </div>
        <div className="reviews-source-score-row">
          {hasScore ? (
            <>
              <strong>{formatScore(source.score)}</strong>
              <span> / {source.maxScore}</span>
            </>
          ) : (
            <strong className="reviews-source-no-score">
              Sin datos disponibles
            </strong>
          )}
        </div>
        {classification && (
          <p
            className={`reviews-source-classification is-${classification.tone}`}
            style={
              {
                "--reviews-score-color": classification.color,
              } as CSSProperties
            }
          >
            <span className="reviews-source-classification-dot" />
            {classification.label}
          </p>
        )}
        {source.count !== null && source.count > 0 ? (
          <p>
            {formatNumber(source.count)} {meta.countLabel}
          </p>
        ) : (
          <p className="reviews-muted-copy">Conteo no disponible</p>
        )}
      </div>
    </article>
  );
}

function SteamReviewCard({
  review,
}: {
  review: GameReviewsSummary["featuredReviews"][number];
}) {
  return (
    <article className="steam-review-card">
      <div className="steam-review-card-header">
        <div className="steam-review-verdict">
          <span
            className={`steam-review-verdict-mark ${review.recommended ? "is-positive" : "is-negative"}`}
          >
            <ReviewVerdictIcon recommended={review.recommended} />
          </span>
          <div>
            <strong>
              {review.recommended ? "Recomendada" : "No recomendada"}
            </strong>
            <span>{review.author}</span>
          </div>
        </div>
        <time dateTime={review.createdAt ?? undefined}>
          {formatReviewDate(review.createdAt)}
        </time>
      </div>
      <div className="steam-review-meta">
        <span>
          {review.playtimeHours === null
            ? "Horas no disponibles"
            : `${formatHours(review.playtimeHours)} jugadas`}
        </span>
        <span>{formatLanguage(review.language)}</span>
      </div>
      <p className="steam-review-text">{review.text}</p>
      {review.helpfulVotes !== null && (
        <p className="steam-review-helpful">
          {formatNumber(review.helpfulVotes)} votos útiles
        </p>
      )}
    </article>
  );
}

function ReviewVerdictIcon({ recommended }: { recommended: boolean }) {
  return (
    <svg
      className={`steam-review-thumb ${recommended ? "is-positive" : "is-negative"}`}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M7.5 10.5H4.25A1.75 1.75 0 0 0 2.5 12.25v7.5a1.75 1.75 0 0 0 1.75 1.75H7.5v-11Z" />
      <path d="M7.5 10.5 12.2 3a2.05 2.05 0 0 1 3.8 1.08V8h3.45a2.05 2.05 0 0 1 2 2.48l-1.42 7.2a2.05 2.05 0 0 1-2 1.65H7.5v-8.83Z" />
    </svg>
  );
}

function SteamReviewsLink({ steamAppId }: { steamAppId: number | null }) {
  const url = steamAppId
    ? `https://store.steampowered.com/app/${steamAppId}/`
    : null;
  return (
    <button
      type="button"
      className="reviews-steam-link"
      disabled={!url}
      onClick={() => void openSafeUrl(url)}
    >
      <span>Ver todas las reseñas en Steam</span>
      <span aria-hidden="true">↗</span>
    </button>
  );
}

function SteamSummaryCard({ summary }: { summary: GameReviewsSummary }) {
  const historical = summary.steamHistorical;
  const recent = summary.steamRecent;
  const distribution = historical ?? recent;
  const positivePercent =
    historical?.positivePercent ?? recent?.positivePercent ?? null;
  const totalCount = historical?.totalCount ?? recent?.totalCount ?? null;
  const rows = distributionRows(distribution);
  const hasSummary = positivePercent !== null || totalCount !== null;

  return (
    <aside className="reviews-column reviews-summary-column">
      <div className="reviews-column-heading">
        <p className="reviews-eyebrow">Steam</p>
        <h2>Resumen de reseñas</h2>
      </div>
      {hasSummary ? (
        <>
          <div className="reviews-summary-overview">
            <div className="reviews-summary-score">
              {positivePercent === null ? (
                <strong className="reviews-summary-no-data">Sin datos</strong>
              ) : (
                <strong>{formatPercent(positivePercent)}</strong>
              )}
              <span>Positivas</span>
              <small>
                {totalCount === null
                  ? "Total no disponible"
                  : `${formatNumber(totalCount)} reseñas`}
              </small>
            </div>
            <div
              className="reviews-donut"
              style={donutStyle(positivePercent)}
              aria-label={
                positivePercent === null
                  ? "Distribución no disponible"
                  : `${formatPercent(positivePercent)} de reseñas positivas`
              }
              role="img"
            >
              <span>
                {positivePercent === null
                  ? "—"
                  : formatPercent(positivePercent)}
              </span>
            </div>
          </div>
          {rows.length > 0 ? (
            <div className="reviews-distribution-list">
              {rows.map((row) => (
                <div className="reviews-distribution-row" key={row.label}>
                  <span>
                    <i className={`reviews-distribution-dot is-${row.tone}`} />
                    {row.label}
                  </span>
                  <strong>
                    {formatNumber(row.count)} ({formatPercent(row.percent)})
                  </strong>
                </div>
              ))}
            </div>
          ) : (
            <p className="reviews-muted-copy">Distribución no disponible.</p>
          )}
          {(recent || historical) && (
            <div className="reviews-period-grid">
              <DistributionPeriod label="Recientes" value={recent} />
              <DistributionPeriod label="Históricas" value={historical} />
            </div>
          )}
        </>
      ) : (
        <ReviewEmptyState
          title="Sin resumen disponible"
          message="Steam no ha devuelto suficientes datos para construir la distribución."
        />
      )}
    </aside>
  );
}

function DistributionPeriod({
  label,
  value,
}: {
  label: string;
  value: ReviewDistribution | null;
}) {
  const percent = value?.positivePercent ?? null;
  return (
    <div className="reviews-period-card">
      <span>{label}</span>
      <strong>{percent === null ? "Sin datos" : formatPercent(percent)}</strong>
      <small>
        {value?.totalCount === null || value?.totalCount === undefined
          ? "Total no disponible"
          : `${formatNumber(value.totalCount)} reseñas`}
      </small>
    </div>
  );
}

function ReviewLoadingState() {
  return (
    <section className="reviews-view reviews-loading" aria-busy="true">
      <div className="reviews-grid">
        <ReviewSkeleton className="reviews-skeleton-sources" lines={5} />
        <ReviewSkeleton className="reviews-skeleton-featured" lines={9} />
        <ReviewSkeleton className="reviews-skeleton-summary" lines={7} />
      </div>
    </section>
  );
}

function ReviewSkeleton({
  className,
  lines,
}: {
  className: string;
  lines: number;
}) {
  return (
    <div className={`reviews-skeleton ${className}`}>
      <span className="reviews-skeleton-heading" />
      {Array.from({ length: lines }, (_, index) => (
        <span className="reviews-skeleton-line" key={index} />
      ))}
    </div>
  );
}

function ReviewErrorState({ onRetry }: { onRetry: () => void }) {
  return (
    <section className="reviews-view reviews-state-view">
      <div className="reviews-state-card">
        <span className="reviews-state-mark" aria-hidden="true">
          !
        </span>
        <h2>No se pudieron cargar las reseñas</h2>
        <p>El servicio de reseñas no está disponible en este momento.</p>
        <button
          type="button"
          className="reviews-state-action"
          onClick={onRetry}
        >
          Reintentar
        </button>
      </div>
    </section>
  );
}

function ReviewEmptyState({
  title,
  message,
}: {
  title: string;
  message: string;
}) {
  return (
    <div className="reviews-empty-state">
      <span className="reviews-empty-mark" aria-hidden="true">
        —
      </span>
      <h3>{title}</h3>
      <p>{message}</p>
    </div>
  );
}

function distributionRows(distribution: ReviewDistribution | null) {
  if (!distribution) return [];
  const rows = [
    {
      label: "Positivas",
      count: distribution.positiveCount,
      percent: distribution.positivePercent,
      tone: "positive",
    },
    {
      label: "Negativas",
      count: distribution.negativeCount,
      percent: distribution.negativePercent,
      tone: "negative",
    },
  ] as const;
  return rows.filter(
    (row): row is (typeof rows)[number] & { count: number; percent: number } =>
      row.count !== null && row.count > 0 && row.percent !== null,
  );
}

function donutStyle(positivePercent: number | null) {
  if (positivePercent === null) {
    return { background: "conic-gradient(#33445b 0deg 360deg)" };
  }
  const angle = Math.min(100, Math.max(0, positivePercent)) * 3.6;
  return {
    background: `conic-gradient(#73a9ff 0deg ${angle}deg, #f15b62 ${angle}deg 360deg)`,
  };
}

function formatScore(value: number | null): string {
  return value === null
    ? "—"
    : Number.isInteger(value)
      ? String(value)
      : value.toFixed(1);
}

function formatPercent(value: number): string {
  return `${Number.isInteger(value) ? value : value.toFixed(1)}%`;
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat("es-419").format(value);
}

function formatHours(value: number): string {
  return `${value.toFixed(value % 1 === 0 ? 0 : 1)} h`;
}

function formatReviewDate(value: string | null): string {
  if (!value) return "Fecha no disponible";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Fecha no disponible";
  return new Intl.DateTimeFormat("es-419", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  })
    .format(date)
    .replace(".", "");
}

function formatLanguage(value: string | null): string {
  if (!value) return "Idioma no disponible";
  return value
    .replace(/[-_]/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

async function openSafeUrl(value: string | null): Promise<void> {
  if (!value) return;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:") return;
    await openUrl(url.toString());
  } catch {
    // External links are intentionally best effort; no UI state is needed.
  }
}
