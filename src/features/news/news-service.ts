import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  NewsFeedViewModel,
  NewsFilter,
  NewsRefreshResult,
  NewsSyncState,
  TranslationBatchSummary,
} from "./news-types";
import { filterCategories } from "./news-types";

const NEWS_TARGET_LANGUAGE = "es-419";
const SAFE_NEWS_HOSTS = [
  "steampowered.com",
  "steamcommunity.com",
  "steamstore-a.akamaihd.net",
];

export const newsService = {
  getFeed(gameId: string, filter: NewsFilter): Promise<NewsFeedViewModel> {
    return invoke<NewsFeedViewModel>("get_game_news_feed", {
      gameId,
      categories: filterCategories(filter),
      limit: 10,
      targetLanguage: NEWS_TARGET_LANGUAGE,
    });
  },

  refresh(gameId: string, forceRefresh = true): Promise<NewsRefreshResult> {
    return invoke<NewsRefreshResult>("refresh_game_news", {
      gameId,
      count: 20,
      maxLength: 8_000,
      forceRefresh,
    });
  },

  getSyncState(gameId: string): Promise<NewsSyncState | null> {
    return invoke<NewsSyncState | null>("get_game_news_sync_state", { gameId });
  },

  translate(newsItemIds: string[]): Promise<TranslationBatchSummary> {
    return invoke<TranslationBatchSummary>("translate_news_items", {
      newsItemIds,
      targetLanguage: NEWS_TARGET_LANGUAGE,
      forceRetranslate: false,
    });
  },

  async openSource(sourceUrl: string): Promise<boolean> {
    const parsed = safeNewsUrl(sourceUrl);
    if (!parsed) return false;
    await openUrl(parsed.toString());
    return true;
  },
};

export function safeNewsUrl(value: string): URL | null {
  try {
    const url = new URL(value);
    if (url.protocol !== "https:") return null;
    if (
      !SAFE_NEWS_HOSTS.some(
        (host) => url.hostname === host || url.hostname.endsWith(`.${host}`),
      )
    ) {
      return null;
    }
    return url;
  } catch {
    return null;
  }
}

export function newsErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  switch (code) {
    case "STEAM_NEWS_OFFLINE":
    case "STEAM_NEWS_TIMEOUT":
      return "Steam no está disponible; se muestran las noticias guardadas.";
    case "STEAM_METADATA_NOT_AVAILABLE":
      return "Este juego todavía no tiene información de Steam suficiente.";
    case "STEAM_NEWS_INVALID_RESPONSE":
      return "Steam devolvió noticias no válidas.";
    default:
      return "No se pudieron cargar las noticias.";
  }
}
