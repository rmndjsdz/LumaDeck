import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  ArtworkApplyRequest,
  ArtworkApplyResult,
  ArtworkSearchRequest,
  ArtworkSearchResult,
  ArtworkSlot,
} from "./artwork-types";

export const artworkService = {
  search(request: ArtworkSearchRequest): Promise<ArtworkSearchResult> {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return Promise.reject(new Error("ARTWORK_RUNTIME_UNAVAILABLE"));
    }
    return invoke<ArtworkSearchResult>("search_steamgriddb_artwork", {
      request,
    });
  },
  cancel(): Promise<void> {
    return invoke<void>("cancel_steamgriddb_artwork_search");
  },
  apply(request: ArtworkApplyRequest): Promise<ArtworkApplyResult> {
    return invoke<ArtworkApplyResult>("apply_steamgriddb_artwork", { request });
  },
  restore(gameId: string, slot: ArtworkSlot): Promise<void> {
    return invoke<void>("restore_steamgriddb_artwork", { gameId, slot });
  },
  current(gameId: string, slot: ArtworkSlot): Promise<string | null> {
    return invoke<string | null>("get_current_steamgriddb_artwork", {
      gameId,
      slot,
    }).then((value) => (value ? resolveArtworkAssetUrl(value) : null));
  },
};

function resolveArtworkAssetUrl(value: string): string {
  if (/^(https?:|data:|blob:|asset:|http:\/\/asset\.localhost)/i.test(value)) {
    return value;
  }
  return convertFileSrc(value);
}

export function artworkErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  switch (code) {
    case "ARTWORK_RUNTIME_UNAVAILABLE":
      return "El selector de arte requiere la aplicacion de escritorio.";
    case "ACCOUNT_NOT_CONFIGURED":
    case "CREDENTIAL_UNAVAILABLE":
      return "Configura una API Key de SteamGridDB antes de buscar arte.";
    case "ARTWORK_CREDENTIAL_REJECTED":
      return "La API Key de SteamGridDB fue rechazada.";
    case "GAME_NOT_FOUND":
      return "No se encontro el juego en la biblioteca local.";
    case "ARTWORK_GAME_IDENTITY_INVALID":
      return "Este juego no tiene una identidad Steam valida para buscar arte.";
    case "ARTWORK_GAME_NOT_FOUND":
      return "No se encontro este juego en SteamGridDB.";
    case "ARTWORK_GAME_AMBIGUOUS":
      return "Hay varias coincidencias para este juego. Se requiere una seleccion manual.";
    case "ARTWORK_SOURCE_OFFLINE":
      return "SteamGridDB no esta disponible en este momento.";
    case "ARTWORK_SOURCE_TIMEOUT":
      return "La consulta a SteamGridDB tardo demasiado.";
    case "ARTWORK_RATE_LIMITED":
      return "SteamGridDB limito temporalmente las consultas. Intenta de nuevo mas tarde.";
    case "ARTWORK_INVALID_RESPONSE":
    case "ARTWORK_SOURCE_ERROR":
      return "SteamGridDB devolvio una respuesta no valida.";
    case "ARTWORK_TEMPORARY_CACHE_UNAVAILABLE":
      return "La cache temporal de SteamGridDB no esta disponible.";
    case "ARTWORK_REQUEST_INVALID":
      return "La solicitud de arte no es valida para este juego.";
    case "ARTWORK_COMMAND_NOT_FOUND":
      return "La instalacion esta desactualizada. Instala el release mas reciente.";
    case "ARTWORK_CANDIDATE_EXPIRED":
      return "El candidato expiro. Vuelve a consultar los resultados.";
    case "ARTWORK_CANDIDATE_INVALID":
      return "El candidato ya no pertenece a esta consulta.";
    case "ARTWORK_SEARCH_CANCELLED":
      return "La consulta fue cancelada.";
    case "ARTWORK_HOST_NOT_ALLOWED":
      return "La imagen proviene de un host no permitido.";
    case "ARTWORK_DOWNLOAD_OFFLINE":
      return "No se pudo descargar el arte seleccionado.";
    case "ARTWORK_DOWNLOAD_TIMEOUT":
      return "La descarga del arte tardo demasiado.";
    case "ARTWORK_TOO_LARGE":
      return "La imagen seleccionada excede el tamano permitido.";
    case "ARTWORK_IMAGE_INVALID":
      return "El archivo descargado no es una imagen valida.";
    case "ARTWORK_ANIMATION_UNSUPPORTED":
      return "El arte animado todavia no es compatible.";
    case "ARTWORK_DIMENSIONS_INVALID":
      return "Las dimensiones de la imagen no son validas.";
    case "ARTWORK_COMPRESSION_ERROR":
      return "No se pudo comprimir la imagen seleccionada.";
    case "ARTWORK_STORAGE_ERROR":
      return "No se pudo guardar el arte en el almacenamiento local.";
    case "DATABASE_ERROR":
      return "La base de datos local no pudo preparar la consulta.";
    default:
      if (/search_steamgriddb_artwork.*not found/i.test(code)) {
        return "La instalacion esta desactualizada. Instala el release mas reciente.";
      }
      return code && code !== "[object Object]"
        ? `No se pudo consultar SteamGridDB. Codigo: ${code.slice(0, 120)}`
        : "No se pudo consultar SteamGridDB.";
  }
}
