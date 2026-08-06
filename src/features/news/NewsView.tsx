import { useEffect, useState } from "react";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { NavigationRow } from "../../ui/navigation/layouts/NavigationRow";
import { ScrollRestoration } from "../../ui/navigation/scroll/scroll-restoration";
import { useNavigation } from "../../ui/navigation/navigation-context";
import {
  useNewsFeed,
  useRefreshGameNews,
  useTranslateVisibleNews,
} from "./news-query";
import { newsErrorMessage, newsService } from "./news-service";
import {
  categoryLabel,
  formatNewsDate,
  NEWS_FILTERS,
  type NewsFilter,
  type NewsItemViewModel,
} from "./news-types";

export function NewsView({ game }: { game: Game }) {
  const { engine } = useNavigation();
  const [filter, setFilter] = useState<NewsFilter>("all");
  const [detail, setDetail] = useState<NewsItemViewModel | null>(null);
  const feedQuery = useNewsFeed(game.id, filter);
  const refreshMutation = useRefreshGameNews(game.id, filter);
  const translationMutation = useTranslateVisibleNews(feedQuery.data);

  useEffect(() => {
    setFilter("all");
    setDetail(null);
  }, [game.id]);

  const openDetail = (item: NewsItemViewModel) => {
    const openerFocusId =
      engine.getActiveFocusId() ?? `news-card-${item.newsItemId}`;
    engine.prepareScopeOpen("details-news-detail", openerFocusId);
    setDetail(item);
  };

  const closeDetail = () => {
    engine.requestScopeRestore(
      "details-news-detail",
      "details",
      `news-detail-close-${game.id}`,
    );
    setDetail(null);
  };

  if (detail) {
    return <NewsDetail game={game} item={detail} onClose={closeDetail} />;
  }

  const data = feedQuery.data;
  const cards = data?.items ?? [];
  const secondaryItems = data?.secondaryItems ?? [];
  const firstCardId = cards[0]
    ? `news-card-${cards[0].newsItemId}`
    : "news-filter-all";
  const errorMessage = feedQuery.error
    ? newsErrorMessage(feedQuery.error)
    : null;

  return (
    <section className="news-view" aria-labelledby="news-heading">
      <main className="news-main-column">
        <div className="news-section-heading">
          <h2 id="news-heading">Noticias destacadas</h2>
          <NewsRefreshButton
            isRefreshing={refreshMutation.isPending}
            onRefresh={() => refreshMutation.mutate()}
            navigation={{ down: "news-hero-open" }}
          />
        </div>
        <NewsStatus
          isStale={data?.isStale ?? false}
          isRefreshing={refreshMutation.isPending}
          isTranslating={translationMutation.isPending}
          hasTranslationFailure={translationMutation.isError}
          errorMessage={errorMessage}
        />
        {feedQuery.isLoading && !data ? (
          <NewsLoadingState />
        ) : feedQuery.isError && !data ? (
          <NewsErrorState
            message={errorMessage ?? "No se pudieron cargar las noticias."}
            onRetry={() => void feedQuery.refetch()}
          />
        ) : data?.totalCount === 0 ? (
          <NewsEmptyState />
        ) : (
          <>
            <NewsHero
              item={data?.hero ?? null}
              fallbackImageUrl={game.backgroundUrl}
              fallbackImageUrls={game.screenshots}
              firstCardId={firstCardId}
              onOpen={openDetail}
            />
            <div className="news-pagination" aria-hidden="true">
              {[0, 1, 2, 3, 4].map((index) => (
                <span
                  key={index}
                  className={index === 0 ? "is-active" : undefined}
                />
              ))}
            </div>
            <div className="news-section-heading news-all-heading">
              <h3>Todas las noticias</h3>
            </div>
            {cards.length > 0 ? (
              <NavigationGrid
                className="news-card-grid"
                groupId="news-card-grid"
                columns={4}
                itemCount={cards.length}
                resolveFocusId={(index) =>
                  `news-card-${cards[index]?.newsItemId ?? ""}`
                }
              >
                {cards.map((item, index) => (
                  <NewsCard
                    key={item.newsItemId}
                    item={item}
                    fallbackImageUrl={game.backgroundUrl}
                    fallbackImageUrls={game.screenshots}
                    gridIndex={index}
                    onOpen={openDetail}
                  />
                ))}
              </NavigationGrid>
            ) : (
              <NewsFilteredEmptyState />
            )}
          </>
        )}
      </main>
      <aside className="news-aside" aria-labelledby="news-filters-heading">
        <h2 id="news-filters-heading">Filtrar noticias</h2>
        <NavigationRow className="news-filter-row" groupId="news-filters">
          {NEWS_FILTERS.map((candidate) => (
            <Focusable
              key={candidate.id}
              focusId={`news-filter-${candidate.id}`}
              scopeId="details"
              className="news-filter-button"
              ariaPressed={filter === candidate.id}
              onConfirm={() => setFilter(candidate.id)}
              navigation={{
                up: "news-hero-open",
                down: secondaryItems[0]
                  ? `news-secondary-${secondaryItems[0].newsItemId}`
                  : firstCardId,
              }}
            >
              {candidate.label}
            </Focusable>
          ))}
        </NavigationRow>
        <ScrollRestoration
          scopeId={`news-secondary-${game.id}-${filter}`}
          className="news-secondary-scroll"
        >
          {secondaryItems.length > 0 ? (
            <div className="news-secondary-list">
              {secondaryItems.map((item, index) => (
                <NewsSecondaryItem
                  key={item.newsItemId}
                  item={item}
                  fallbackImageUrl={game.backgroundUrl}
                  fallbackImageUrls={game.screenshots}
                  previousId={
                    index > 0
                      ? `news-secondary-${secondaryItems[index - 1].newsItemId}`
                      : undefined
                  }
                  nextId={
                    index < secondaryItems.length - 1
                      ? `news-secondary-${secondaryItems[index + 1].newsItemId}`
                      : undefined
                  }
                  onOpen={openDetail}
                />
              ))}
            </div>
          ) : (
            <p className="news-aside-empty">No hay noticias en este filtro.</p>
          )}
        </ScrollRestoration>
      </aside>
    </section>
  );
}

function NewsHero({
  item,
  fallbackImageUrl,
  fallbackImageUrls,
  firstCardId,
  onOpen,
}: {
  item: NewsItemViewModel | null;
  fallbackImageUrl: string;
  fallbackImageUrls: string[];
  firstCardId: string;
  onOpen: (item: NewsItemViewModel) => void;
}) {
  if (!item) {
    return (
      <div className="news-hero news-hero-placeholder">
        No hay noticias destacadas.
      </div>
    );
  }
  return (
    <Focusable
      focusId="news-hero-open"
      scopeId="details"
      className="news-hero"
      navigation={{ right: "news-filter-all", down: firstCardId }}
      onConfirm={() => onOpen(item)}
      ariaLabel={`Leer noticia destacada: ${item.displayTitle}`}
    >
      <NewsImage
        item={item}
        fallbackImageUrl={fallbackImageUrl}
        fallbackImageUrls={fallbackImageUrls}
        variant="hero"
      />
      <div className="news-hero-copy">
        <NewsMeta item={item} hero />
        <h3>{item.displayTitle}</h3>
        <p>
          {item.displaySummary ??
            "Consulta todos los detalles de esta noticia."}
        </p>
        <span className="news-hero-cta" aria-hidden="true">
          <span className="news-button-input">A</span>
          Leer más
        </span>
      </div>
    </Focusable>
  );
}

function NewsCard({
  item,
  fallbackImageUrl,
  fallbackImageUrls,
  gridIndex,
  onOpen,
}: {
  item: NewsItemViewModel;
  fallbackImageUrl: string;
  fallbackImageUrls: string[];
  gridIndex: number;
  onOpen: (item: NewsItemViewModel) => void;
}) {
  return (
    <Focusable
      focusId={`news-card-${item.newsItemId}`}
      scopeId="details"
      className="news-card"
      gridIndex={gridIndex}
      navigation={{ up: gridIndex === 0 ? "news-hero-open" : undefined }}
      onConfirm={() => onOpen(item)}
      ariaLabel={`Leer noticia: ${item.displayTitle}`}
    >
      <NewsImage
        item={item}
        fallbackImageUrl={fallbackImageUrl}
        fallbackImageUrls={fallbackImageUrls}
        variant="card"
      />
      <div className="news-card-copy">
        <NewsMeta item={item} />
        <h4>{item.displayTitle}</h4>
        <p>{item.displaySummary ?? "Sin resumen disponible."}</p>
        <NewsComments count={item.commentCount} />
      </div>
    </Focusable>
  );
}

function NewsSecondaryItem({
  item,
  fallbackImageUrl,
  fallbackImageUrls,
  previousId,
  nextId,
  onOpen,
}: {
  item: NewsItemViewModel;
  fallbackImageUrl: string;
  fallbackImageUrls: string[];
  previousId?: string;
  nextId?: string;
  onOpen: (item: NewsItemViewModel) => void;
}) {
  return (
    <Focusable
      focusId={`news-secondary-${item.newsItemId}`}
      scopeId="details"
      className="news-secondary-item"
      navigation={{
        up: previousId ?? "news-filter-all",
        down: nextId,
        left: "news-card-0",
      }}
      onConfirm={() => onOpen(item)}
      ariaLabel={`Leer noticia: ${item.displayTitle}`}
    >
      <NewsImage
        item={item}
        fallbackImageUrl={fallbackImageUrl}
        fallbackImageUrls={fallbackImageUrls}
        variant="secondary"
      />
      <div className="news-secondary-copy">
        <NewsMeta item={item} />
        <h3>{item.displayTitle}</h3>
        <p>{item.displaySummary ?? "Sin resumen disponible."}</p>
        <NewsComments count={item.commentCount} />
      </div>
    </Focusable>
  );
}

function NewsDetail({
  game,
  item,
  onClose,
}: {
  game: Game;
  item: NewsItemViewModel;
  onClose: () => void;
}) {
  const [sourceError, setSourceError] = useState(false);
  const content = plainNewsText(
    item.displayContent ??
      item.originalContent ??
      item.displaySummary ??
      "Sin contenido disponible.",
  );
  return (
    <FocusScope
      scopeId="details-news-detail"
      parentScopeId="details"
      initialFocusId="news-detail-back"
      restoreFocus
      rememberScroll
      trapFocus
      activateOnMount
      onBack={() => {
        onClose();
        return true;
      }}
    >
      <ScrollRestoration
        scopeId={`news-detail-content-${game.id}`}
        className="news-detail-scroll"
      >
        <article className="news-detail" aria-labelledby="news-detail-heading">
          <Focusable
            focusId="news-detail-back"
            scopeId="details-news-detail"
            className="news-detail-back"
            onConfirm={onClose}
            ariaLabel="Volver a Noticias"
          >
            ← Volver
          </Focusable>
          <div className="news-detail-image-wrap">
            <NewsImage
              item={item}
              fallbackImageUrl={game.backgroundUrl}
              fallbackImageUrls={game.screenshots}
              variant="detail"
            />
          </div>
          <NewsMeta item={item} />
          <h1 id="news-detail-heading">{item.displayTitle}</h1>
          <p className="news-detail-summary">{item.displaySummary ?? ""}</p>
          <p className="news-detail-content">{content}</p>
          {!sourceError && (
            <Focusable
              focusId="news-detail-source"
              scopeId="details-news-detail"
              className="news-detail-source"
              onConfirm={() => {
                void newsService.openSource(item.sourceUrl).then((opened) => {
                  if (!opened) setSourceError(true);
                });
              }}
            >
              Ver fuente original
            </Focusable>
          )}
          {sourceError && (
            <p className="news-detail-source-error">
              La fuente no está disponible.
            </p>
          )}
        </article>
      </ScrollRestoration>
    </FocusScope>
  );
}

function NewsMeta({
  item,
  hero = false,
}: {
  item: NewsItemViewModel;
  hero?: boolean;
}) {
  return (
    <div className={`news-meta${hero ? " news-meta-hero" : ""}`}>
      <span className="news-category">{categoryLabel(item.category)}</span>
      <span>{formatNewsDate(item.publishedAt)}</span>
      {item.hasTranslation && (
        <span className="news-translation-indicator">ES</span>
      )}
    </div>
  );
}

function NewsImage({
  item,
  fallbackImageUrl,
  fallbackImageUrls,
  variant,
}: {
  item: NewsItemViewModel;
  fallbackImageUrl: string;
  fallbackImageUrls: string[];
  variant: "card" | "secondary" | "detail" | "hero";
}) {
  const rotatedScreenshots = rotateImageSources(
    fallbackImageUrls,
    item.newsItemId,
  );
  const sources = [
    item.imageUrl,
    item.thumbnailUrl,
    ...rotatedScreenshots,
    fallbackImageUrl,
  ].filter(
    (source, index, all): source is string =>
      Boolean(source) && all.indexOf(source) === index,
  );
  const [sourceIndex, setSourceIndex] = useState(0);
  useEffect(
    () => setSourceIndex(0),
    [
      fallbackImageUrl,
      fallbackImageUrls,
      item.imageUrl,
      item.newsItemId,
      item.thumbnailUrl,
    ],
  );
  const source = sources[sourceIndex];
  return source ? (
    <img
      src={source}
      alt=""
      className={`news-image news-image-${variant}`}
      loading={variant === "detail" ? "eager" : "lazy"}
      draggable={false}
      onError={() => setSourceIndex((current) => current + 1)}
    />
  ) : (
    <div
      className={`news-image news-image-${variant} news-image-placeholder`}
      aria-hidden="true"
    />
  );
}

function rotateImageSources(sources: string[], key: string): string[] {
  if (sources.length < 2) return sources;
  const offset =
    [...key].reduce((total, character) => total + character.charCodeAt(0), 0) %
    sources.length;
  return [...sources.slice(offset), ...sources.slice(0, offset)];
}

function NewsComments({ count }: { count: number | null }) {
  return count === null ? null : (
    <span className="news-comments">◌ {count}</span>
  );
}

function NewsRefreshButton({
  isRefreshing,
  onRefresh,
  navigation,
}: {
  isRefreshing: boolean;
  onRefresh: () => void;
  navigation: { down: string };
}) {
  return (
    <Focusable
      focusId="news-refresh"
      scopeId="details"
      className="news-refresh-button"
      disabled={isRefreshing}
      navigation={navigation}
      onConfirm={onRefresh}
      ariaLabel={isRefreshing ? "Actualizando noticias" : "Actualizar noticias"}
      ariaPressed={isRefreshing}
    >
      ↻
    </Focusable>
  );
}

function NewsStatus({
  isStale,
  isRefreshing,
  isTranslating,
  hasTranslationFailure,
  errorMessage,
}: {
  isStale: boolean;
  isRefreshing: boolean;
  isTranslating: boolean;
  hasTranslationFailure: boolean;
  errorMessage: string | null;
}) {
  const message = isRefreshing
    ? "Actualizando noticias…"
    : isTranslating
      ? "Preparando traducción…"
      : isStale
        ? "Mostrando noticias guardadas."
        : hasTranslationFailure
          ? "Algunas noticias se muestran en su idioma original."
          : errorMessage;
  return message ? (
    <p
      className="news-status"
      role="status"
      aria-live="polite"
      aria-busy={isRefreshing || isTranslating}
    >
      {message}
    </p>
  ) : null;
}

function NewsLoadingState() {
  return (
    <div
      className="news-loading-state"
      aria-busy="true"
      aria-label="Cargando noticias"
    >
      <div className="news-loading-hero" />
      <div className="news-loading-cards">
        {[0, 1, 2, 3].map((index) => (
          <div key={index} />
        ))}
      </div>
    </div>
  );
}

function NewsErrorState({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="news-state-panel">
      <h3>No se pudieron cargar las noticias</h3>
      <p>{message}</p>
      <Focusable
        focusId="news-retry"
        scopeId="details"
        className="primary-button"
        onConfirm={onRetry}
      >
        Reintentar
      </Focusable>
    </div>
  );
}

function NewsEmptyState() {
  return (
    <div className="news-state-panel">
      <h3>Aún no hay noticias</h3>
      <p>Actualiza este juego para consultar sus publicaciones de Steam.</p>
    </div>
  );
}

function NewsFilteredEmptyState() {
  return (
    <p className="news-filtered-empty">No hay noticias en esta categoría.</p>
  );
}

function plainNewsText(value: string): string {
  return value
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;|&apos;/gi, "'")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
