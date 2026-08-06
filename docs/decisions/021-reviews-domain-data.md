# Reviews domain and data integration

## Inspección de referencia

En Ludex se encontró:

- `server/services/steam-service.js` con el endpoint oficial de Steam
  `store.steampowered.com/appreviews/{appid}`.
- `ludex-next/electron/services/game-detail-service.cjs`, que consulta
  `appdetails`, reseñas recientes de Steam y lee `metacritic.score` desde la
  respuesta de Steam Store.
- `ludex-next/src/shared/contracts.ts` y el panel de reseñas, con los campos
  `reviewPercent`, `reviewCount`, `reviewLabel`, `reviews`, `helpful` y
  `metacriticScore`.

No se encontró una implementación funcional reutilizable de OpenCritic en
Ludex; el panel solo mostraba la fuente como no disponible. Tampoco se
copiaron carpetas ni se modificó el repositorio de referencia.

## Decisiones de LumaDeck

- El identificador canónico es `game.id`; el proveedor Steam se resuelve por
  `steam_app_id`/`Game.details.steam.appId`.
- El backend Tauri resuelve el juego desde SQLite y ejecuta una sola operación
  de dominio. Dentro de ella, Metacritic vía Steam Store, OpenCritic y Steam
  se consultan en paralelo con el mismo cliente `reqwest`.
- Los fallos se convierten en errores por proveedor. Una fuente caída no
  cancela las demás.
- Metacritic usa `https://store.steampowered.com/api/appdetails` y normaliza
  `metacritic.score` y `metacritic.url`.
- Steam usa dos consultas oficiales a
  `https://store.steampowered.com/appreviews/{appid}`: `filter=all` para el
  histórico y `filter=recent` para lo reciente.
- OpenCritic usa `https://api.opencritic.com/api/game/search` y
  `https://api.opencritic.com/api/game/{id}` como integración best-effort.
  Si no responde o no encuentra el juego, el resto de fuentes sigue siendo
  utilizable.
- No se añadieron credenciales, tablas SQLite, stores globales ni un segundo
  cliente HTTP del frontend. La caché y `staleTime`/`gcTime` pertenecen a
  TanStack Query; no hay polling.

## Contrato final

`GameReviewsSummary` expone a consumidores futuros únicamente el dominio:

- estado `success`, `partial`, `no-data`, `identifier-missing` o `error`;
- resumen por fuente con score, máximo, conteo, URL, distribución y error;
- distribución histórica y reciente de Steam;
- reseñas destacadas con texto, recomendación, horas jugadas, fecha, idioma y
  votos útiles;
- `fetchedAt` y errores normalizados.

Los DTO de proveedores se mantienen separados de los parsers y del dominio.

## Archivos

Nuevos:

- `src/features/reviews/reviews-types.ts`
- `src/features/reviews/reviews-parsers.ts`
- `src/features/reviews/reviews-service.ts`
- `src/features/reviews/reviews-query.ts`
- `src/features/reviews/reviews-parsers.test.ts`
- `src/features/reviews/reviews-service.test.ts`
- `src/features/reviews/reviews-query.test.ts`
- `src-tauri/src/reviews.rs`

Modificado:

- `src-tauri/src/lib.rs`, para registrar el comando Tauri
  `get_game_reviews_sources`.

No se implementó UI, navegación ni animación.
