import { invoke } from "@tauri-apps/api/core";
import {
  createIdentifierMissingSummary,
  parseGameReviewsSummary,
} from "./reviews-parsers";
import type {
  GameReviewsSummary,
  ReviewSourcesDto,
  ReviewsGame,
} from "./reviews-types";

export interface ReviewsDataSource {
  getSources(game: ReviewsGame, signal?: AbortSignal): Promise<unknown>;
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function abortError(signal: AbortSignal): Error {
  if (signal.reason instanceof Error) return signal.reason;
  return new DOMException("The reviews request was aborted", "AbortError");
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw abortError(signal);
}

function awaitWithAbort<T>(
  promise: Promise<T>,
  signal?: AbortSignal,
): Promise<T> {
  if (!signal) return promise;
  throwIfAborted(signal);
  return new Promise<T>((resolve, reject) => {
    let settled = false;
    const cleanup = () => signal.removeEventListener("abort", onAbort);
    const onAbort = () => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(abortError(signal));
    };
    signal.addEventListener("abort", onAbort, { once: true });
    promise.then(
      (value) => {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(value);
      },
      (error: unknown) => {
        if (settled) return;
        settled = true;
        cleanup();
        reject(error);
      },
    );
  });
}

function emptySources(game: ReviewsGame): ReviewSourcesDto {
  return {
    gameId: game.id,
    title: game.title,
    steamAppId: game.details?.steam?.appId ?? null,
    metacritic: null,
    opencritic: null,
    steam: null,
    errors: [],
  };
}

const defaultDataSource: ReviewsDataSource = {
  async getSources(game, signal) {
    throwIfAborted(signal);
    if (!isTauriRuntime()) return emptySources(game);
    const response = await awaitWithAbort(
      invoke<ReviewSourcesDto>("get_game_reviews_sources", {
        gameId: game.id,
      }),
      signal,
    );
    throwIfAborted(signal);
    return response;
  },
};

export function createReviewsService(
  source: ReviewsDataSource = defaultDataSource,
) {
  return {
    async getGameReviewsSummary(
      game: ReviewsGame,
      signal?: AbortSignal,
    ): Promise<GameReviewsSummary> {
      if (!game.details?.steam?.appId) {
        return createIdentifierMissingSummary(game);
      }
      throwIfAborted(signal);
      const sources = await awaitWithAbort(
        source.getSources(game, signal),
        signal,
      );
      throwIfAborted(signal);
      return parseGameReviewsSummary(sources, game);
    },
  };
}

export const reviewsService = createReviewsService();
