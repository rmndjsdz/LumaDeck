import { Focusable } from "../../ui/navigation/focus/Focusable";
import type { GameReviewsSummary } from "./reviews-types";
import type { GameReviewConsensus } from "./consensus-types";
import "./reviews.css";

interface ConsensusPanelProps {
  summary: GameReviewsSummary;
  consensus: GameReviewConsensus | null;
  aiConfigured: boolean;
  loading: boolean;
  generating: boolean;
  errorMessage: string | null;
  onGenerate: () => void;
  onUpdate: () => void;
  onOpenSettings: () => void;
}

export function ConsensusPanel({
  summary,
  consensus,
  aiConfigured,
  loading,
  generating,
  errorMessage,
  onGenerate,
  onUpdate,
  onOpenSettings,
}: ConsensusPanelProps) {
  const stale = Boolean(
    consensus &&
    summary.inputFingerprint &&
    consensus.inputFingerprint !== summary.inputFingerprint,
  );

  return (
    <section
      className="reviews-column reviews-consensus-column"
      aria-labelledby="consensus-heading"
    >
      <div className="reviews-column-heading">
        <div>
          <h2 id="consensus-heading">
            Consenso <span aria-hidden="true">ⓘ</span>
          </h2>
        </div>
      </div>
      {loading && !consensus ? (
        <ConsensusLoading />
      ) : !aiConfigured && !consensus ? (
        <ConsensusState
          title="Servicios IA no configurados"
          message="Configura OpenRouter en Settings para generar un consenso local con estas fuentes."
          actionLabel="Ir a Settings"
          focusId="reviews-consensus-settings"
          onAction={onOpenSettings}
        />
      ) : !consensus ? (
        <ConsensusState
          title="Consenso no generado"
          message="Sintetiza la opinión de críticos y jugadores mediante IA. El resultado se guarda localmente y no se vuelve a generar salvo que tú lo solicites."
          actionLabel={generating ? "Generando…" : "Generar consenso"}
          focusId="reviews-consensus-generate"
          disabled={generating}
          onAction={onGenerate}
        />
      ) : (
        <ConsensusContent
          consensus={consensus}
          stale={stale}
          generating={generating}
          errorMessage={errorMessage}
          onUpdate={onUpdate}
        />
      )}
      {errorMessage && !consensus && (
        <p className="reviews-consensus-error" role="alert">
          {consensusErrorLabel(errorMessage)}
        </p>
      )}
    </section>
  );
}

function ConsensusContent({
  consensus,
  stale,
  generating,
  errorMessage,
  onUpdate,
}: {
  consensus: GameReviewConsensus;
  stale: boolean;
  generating: boolean;
  errorMessage: string | null;
  onUpdate: () => void;
}) {
  return (
    <>
      <div className="reviews-consensus-rating">
        <div
          className="reviews-consensus-stars"
          aria-label={ratingLabel(consensus.overallRating)}
        >
          {stars(consensus.overallRating)}
        </div>
        <span>
          {consensus.overallRating === null
            ? "Sin puntuación agregada"
            : `${consensus.overallRating.toFixed(1)} / 5`}
        </span>
        <span className="reviews-consensus-divider" aria-hidden="true">
          ·
        </span>
        <p className={`reviews-consensus-agreement is-${consensus.agreement}`}>
          {consensus.agreementLabel}
        </p>
      </div>
      <div className="reviews-consensus-lists">
        <ConsensusList
          title="Lo mejor"
          values={consensus.strengths}
          tone="positive"
        />
        <ConsensusList
          title="Lo menos destacado"
          values={consensus.weaknesses}
          tone="warning"
        />
      </div>
      <div className="reviews-consensus-conclusion">
        <p className="reviews-eyebrow">En pocas palabras</p>
        <p>{consensus.conclusion}</p>
      </div>
      <p className="reviews-consensus-footnote">
        {sourceCountLabel(consensus)} · Generado el{" "}
        {formatGeneratedAt(consensus.generatedAt)}
      </p>
      {(stale || errorMessage) && (
        <p
          className={`reviews-consensus-notice ${errorMessage ? "is-error" : ""}`}
          role={errorMessage ? "alert" : "status"}
        >
          {errorMessage
            ? consensusErrorLabel(errorMessage)
            : "Puede haber información más reciente"}
        </p>
      )}
      <Focusable
        focusId="reviews-consensus-update"
        scopeId="details"
        className="reviews-consensus-action"
        disabled={generating}
        onConfirm={onUpdate}
      >
        {generating
          ? "Actualizando…"
          : stale
            ? "Actualizar consenso"
            : "Actualizar"}
      </Focusable>
    </>
  );
}

function ConsensusList({
  title,
  values,
  tone,
}: {
  title: string;
  values: string[];
  tone: "positive" | "warning";
}) {
  return (
    <div className={`reviews-consensus-list is-${tone}`}>
      <p className="reviews-eyebrow">{title}</p>
      {values.length > 0 ? (
        <ul>
          {values.map((value) => (
            <li key={value}>{value}</li>
          ))}
        </ul>
      ) : (
        <p className="reviews-muted-copy">Sin evidencia suficiente</p>
      )}
    </div>
  );
}

function ConsensusState({
  title,
  message,
  actionLabel,
  focusId,
  disabled = false,
  onAction,
}: {
  title: string;
  message: string;
  actionLabel: string;
  focusId: string;
  disabled?: boolean;
  onAction: () => void;
}) {
  return (
    <div className="reviews-consensus-state">
      <span className="reviews-empty-mark" aria-hidden="true">
        ✦
      </span>
      <h3>{title}</h3>
      <p>{message}</p>
      <Focusable
        focusId={focusId}
        scopeId="details"
        className="reviews-consensus-action"
        disabled={disabled}
        onConfirm={onAction}
      >
        {actionLabel}
      </Focusable>
    </div>
  );
}

function ConsensusLoading() {
  return (
    <div className="reviews-consensus-loading" aria-busy="true">
      <span className="reviews-skeleton-heading" />
      <span className="reviews-skeleton-line" />
      <span className="reviews-skeleton-line" />
      <p>Analizando críticas y reseñas…</p>
    </div>
  );
}

function stars(rating: number | null): string {
  if (rating === null) return "☆☆☆☆☆";
  const rounded = Math.max(0, Math.min(5, Math.round(rating)));
  return `${"★".repeat(rounded)}${"☆".repeat(5 - rounded)}`;
}

function ratingLabel(rating: number | null): string {
  return rating === null
    ? "Sin puntuación"
    : `${rating.toFixed(1)} de 5 estrellas`;
}

function sourceCountLabel(consensus: GameReviewConsensus): string {
  const critics = consensus.sources.criticReviewCount;
  const players = consensus.sources.playerReviewCount;
  if (critics !== null && players !== null)
    return `Basado en ${critics.toLocaleString("es-419")} críticas y ${players.toLocaleString("es-419")} reseñas de jugadores`;
  if (players !== null)
    return `Basado en ${players.toLocaleString("es-419")} reseñas de jugadores`;
  if (critics !== null)
    return `Basado en ${critics.toLocaleString("es-419")} críticas`;
  return `Basado en ${consensus.sources.sampledSteamReviews} reseñas seleccionadas`;
}

function formatGeneratedAt(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "fecha no disponible"
    : new Intl.DateTimeFormat("es-419", {
        day: "numeric",
        month: "long",
        year: "numeric",
      }).format(date);
}

function consensusErrorLabel(error: string): string {
  const labels: Record<string, string> = {
    CONSENSUS_INSUFFICIENT_DATA:
      "No hay suficientes críticas o reseñas para generar un consenso confiable.",
    AI_NOT_CONFIGURED: "Servicios IA no están configurados.",
    AI_AUTHENTICATION_ERROR: "La credencial de OpenRouter no es válida.",
    AI_PAYMENT_REQUIRED:
      "OpenRouter requiere créditos o permisos de facturación para este modelo.",
    AI_RATE_LIMITED:
      "OpenRouter alcanzó su límite temporal. Inténtalo más tarde.",
    AI_TIMEOUT: "OpenRouter agotó el tiempo de espera.",
    AI_MODEL_UNAVAILABLE: "El modelo configurado no está disponible.",
    AI_INVALID_RESPONSE: "OpenRouter devolvió una respuesta inválida.",
    AI_EMPTY_RESPONSE: "OpenRouter no devolvió contenido.",
  };
  return (
    labels[error] ??
    "No se pudo actualizar el consenso. El consenso anterior se conserva."
  );
}
