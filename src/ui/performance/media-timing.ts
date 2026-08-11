import { invoke } from "@tauri-apps/api/core";

export type MediaType = "hero" | "grid" | "screenshot" | "logo";

export type MediaTimingStage =
  | "ASSET_URL_CREATED"
  | "REACT_DATA_READY"
  | "IMG_REQUEST"
  | "IMG_LOAD"
  | "IMG_DECODED"
  | "IMG_ERROR"
  | "DETAILS_OPEN"
  | "DETAILS_LEAVE"
  | "DETAILS_QUERY_FETCH_START"
  | "DETAILS_QUERY_FETCH_END"
  | "DETAILS_QUERY_STATE"
  | "DETAILS_QUERY_EVENT"
  | "MEDIA_IMAGE_MOUNT"
  | "MEDIA_IMAGE_UNMOUNT"
  | "MEDIA_IMAGE_ELEMENT_ATTACHED"
  | "MEDIA_IMAGE_ELEMENT_DETACHED"
  | "BACKGROUND_CACHE_HIT"
  | "BACKGROUND_CACHE_MISS"
  | "BACKGROUND_CACHE_EVICT"
  | "MEDIA_CACHE_HIT"
  | "MEDIA_CACHE_MISS"
  | "MEDIA_CACHE_INSERT"
  | "MEDIA_CACHE_EVICT"
  | "MEDIA_PRELOAD_START"
  | "MEDIA_PRELOAD_READY"
  | "HOME_MEDIA_READY"
  | "DETAILS_MEDIA_READY"
  | "VISUAL_CACHE_HIT"
  | "HOME_DETAILS_HOTSET_START"
  | "HOME_DETAILS_HOTSET_READY"
  | "DETAILS_PRELOAD_START"
  | "DETAILS_DATA_READY"
  | "DETAILS_CRITICAL_READY"
  | "DETAILS_TRANSITION_START";

interface MediaTimingDetails {
  gameId: string;
  type: MediaType;
  path?: string;
  url?: string;
  durationMs?: number;
  detail?: string;
}

const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const traceGameIds = new Set(
  (
    (import.meta.env.VITE_MEDIA_TRACE_GAME_IDS as string | undefined) ??
    (import.meta.env.VITE_MEDIA_TRACE_GAME_ID as string | undefined) ??
    ""
  )
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean),
);

export function recordMediaTiming(
  stage: MediaTimingStage,
  details: MediaTimingDetails,
): void {
  if (
    !import.meta.env.DEV ||
    (traceGameIds.size > 0 && !traceGameIds.has(details.gameId))
  ) {
    return;
  }
  const timestampMs = performance.timeOrigin + performance.now();
  const payload = { stage, timestampMs, ...details };
  console.info("[media-timing]", payload);
  if (isTauri) {
    void invoke("record_media_timing", {
      stage,
      timestampMs,
      gameId: details.gameId,
      mediaType: details.type,
      path: details.path ?? "",
      url: details.url ?? "",
      durationMs: details.durationMs ?? null,
      detail: details.detail ?? "",
    }).catch(() => undefined);
  }
}

export function mediaRequestStarted(): number {
  return performance.now();
}
