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

## Arquitectura de datos

El tab de Reseñas tiene dos niveles de caché distintos:

1. **TanStack Query en el frontend:** caché de consultas en memoria para
   evitar repetir la misma consulta mientras la aplicación está abierta y
   conservar temporalmente el resultado al cambiar de vista.
2. **SQLite en el backend Tauri:** persistencia local de los datos de los
   proveedores y del consenso generado por IA.

Estas capas no son equivalentes. TanStack Query no escribe en disco en la
implementación actual: no se utiliza `localStorage`, IndexedDB ni un
`persistQueryClient`. Al cerrar LumaDeck se pierde su caché en memoria. La
información que sobrevive al cierre está en SQLite.

### Qué significa TanStack Query aquí

TanStack Query administra el ciclo de vida de una consulta asíncrona:

- **`queryKey`:** identifica el resultado. Para las reseñas es
  `['reviews-summary', gameId]`; para el consenso es
  `['reviews-consensus', gameId]`.
- **`queryFn`:** ejecuta la carga. En reseñas llama al comando Tauri
  `get_game_reviews_sources`, que finalmente consulta o reutiliza SQLite y
  los proveedores remotos.
- **`staleTime`:** durante ese tiempo el resultado se considera fresco. No
  significa que se guarde durante ese tiempo en disco.
- **`gcTime`:** cuánto tiempo se conserva en memoria una consulta que ya no
  tiene componentes utilizándola. Cuando termina ese plazo, TanStack Query
  puede retirarla de memoria.

La configuración actual es:

- Resumen de reseñas: `staleTime` de 30 minutos y `gcTime` de 24 horas.
- Consenso: `staleTime: Infinity` y `gcTime` de 24 horas.

Por tanto, los 30 minutos y las 24 horas son políticas de memoria del
frontend, no una política de retención de los datos persistentes. Además,
aunque TanStack Query vuelva a pedir el resumen después de quedar stale, el
backend puede responder desde su caché SQLite.

## Decisiones de LumaDeck

- El identificador canónico es `game.id`; el proveedor Steam se resuelve por
  `steam_app_id`/`Game.details.steam.appId`.
- El backend Tauri resuelve el juego desde SQLite y ejecuta una sola operación
  de dominio. Dentro de ella, Metacritic, OpenCritic y Steam se consultan en
  paralelo con el mismo cliente `reqwest` cuando la política de caché lo
  requiere.
- Los fallos se convierten en errores por proveedor. Una fuente caída no
  cancela las demás.
- Metacritic usa RapidAPI cuando existe una API key de reseñas; sin ella usa
  `https://store.steampowered.com/api/appdetails` y normaliza
  `metacritic.score` y `metacritic.url`.
- Steam usa dos consultas oficiales a
  `https://store.steampowered.com/appreviews/{appid}`: `filter=all` para el
  histórico y `filter=recent` para lo reciente.
- OpenCritic usa RapidAPI con las rutas de búsqueda y detalle como integración
  best-effort. Si no responde o no encuentra el juego, el resto de fuentes
  sigue siendo utilizable.
- No hay polling automático en el tab de Reseñas.

## Persistencia local y políticas de caché

### Caché de proveedores: `game_reviews_cache`

La tabla se crea en la migración SQLite 16 y contiene una fila por juego:

- `metacritic_json` y `metacritic_updated_at`;
- `opencritic_json` y `opencritic_updated_at`;
- `steam_json` y `steam_updated_at`;
- `steam_app_id`, `created_at` y `updated_at`.

Los JSON contienen los DTO normalizados de LumaDeck, no el payload HTTP crudo.
El JSON de Steam incluye los resúmenes histórico/reciente y las reseñas
seleccionadas, con texto, autor, idioma, horas jugadas, fecha y votos útiles.

La política efectiva es:

- **Metacritic:** caché no expirable. Si existe un valor válido para el mismo
  `steam_app_id`, se reutiliza indefinidamente.
- **OpenCritic:** caché no expirable. Si existe un valor válido para el mismo
  `steam_app_id`, se reutiliza indefinidamente.
- **Steam:** caché diaria basada en el día calendario local. Si
  `steam_updated_at` pertenece al día actual, se reutiliza; si pertenece a un
  día anterior, se intenta actualizar.
- **Fallback de Steam:** si la actualización falla y había un valor anterior,
  se devuelve ese valor anterior junto con el error del proveedor.

La política diaria de Steam controla cuándo se vuelve a consultar; no elimina
los valores anteriores. Tampoco existe actualmente una limpieza automática
por edad de `game_reviews_cache`.

### Consenso generado por IA: `game_review_consensus`

La tabla guarda un consenso por juego, incluyendo:

- `consensus_json` con conclusión, fortalezas, debilidades y nivel de acuerdo;
- `generated_at`;
- proveedor y modelo;
- `prompt_version`;
- `input_fingerprint`.

Si se solicita el consenso sin `forceRefresh`, se devuelve el registro
existente. La acción explícita de actualizar fuerza una nueva generación y
reemplaza el registro anterior.

`input_fingerprint` permite identificar qué entradas originaron el consenso,
pero actualmente no se utiliza para invalidarlo automáticamente cuando
cambian las reseñas. El consenso tampoco tiene TTL ni limpieza automática.

### Imágenes

El tab de Reseñas no descarga ni persiste imágenes de los proveedores. Los
logos de Metacritic, OpenCritic y Steam son assets incluidos en el bundle de
la aplicación. El enlace para ver todas las reseñas de Steam abre una URL
externa, pero no crea una caché de imágenes propia.

## Ubicación del almacenamiento

La base SQLite se ubica en el directorio de datos de la aplicación en modo
normal. En modo portable se ubica en `<ejecutable>\\data\\lumadeck.db`. El
directorio de datos también puede contener los logs y otras cachés de LumaDeck,
pero esas cachés no forman parte del tab de Reseñas.

## Limpieza y retención

- No existe un comando ni una acción de UI para limpiar únicamente la caché de
  Reseñas.
- No se encontró una rutina de purga por antigüedad para `game_reviews_cache`
  ni para `game_review_consensus`.
- Las filas están relacionadas con `games` mediante `ON DELETE CASCADE`, por
  lo que se eliminarían si se eliminara el juego de la base de datos. El flujo
  actual no ofrece una eliminación normal de juegos desde la UI.
- Eliminar una API key de RapidAPI no elimina los datos de reseñas ya
  persistidos.

## Contrato final

`GameReviewsSummary` expone a consumidores futuros únicamente el dominio:

- estado `success`, `partial`, `no-data`, `identifier-missing` o `error`;
- resumen por fuente con score, máximo, conteo, URL, distribución y error;
- distribución histórica y reciente de Steam;
- reseñas destacadas con texto, recomendación, horas jugadas, fecha, idioma y
  votos útiles;
- `fetchedAt` y errores normalizados;
- `inputFingerprint` para relacionar el resumen con el consenso.

Los DTO de proveedores se mantienen separados de los parsers y del dominio.

## Archivos relevantes

- `src/features/reviews/reviews-query.ts`: caché de TanStack Query del resumen.
- `src/features/reviews/consensus-query.ts`: caché en memoria del consenso.
- `src/features/reviews/reviews-service.ts`: puente frontend hacia Tauri.
- `src/features/reviews/ReviewsView.tsx`: UI y assets visuales del tab.
- `src-tauri/src/reviews.rs`: consultas y política de caché por proveedor.
- `src-tauri/src/lib.rs`: coordinación, lectura/escritura de caché y comandos.
- `src-tauri/src/settings/database.rs`: migraciones y tablas SQLite.
- `src-tauri/src/settings/repositories.rs`: lectura y escritura de los
  registros persistentes.
