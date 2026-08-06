# 018 — Steam News Provider V1

## Estado

`STEAM NEWS PROVIDER V1: IMPLEMENTED`

El dominio y almacenamiento de Noticias V1 continúan implementados. El
proveedor de traducción, la UI de Noticias y la validación QA en la aplicación
real permanecen pendientes.

## Flujo implementado

`refresh_game_news` resuelve el `steam_app_id` exclusivamente desde
`game_details.steam_app_id`. No usa títulos ni mappings heurísticos. El flujo
es:

```text
gameId
  -> settings::get_steam_app_id
  -> SteamNewsProvider
  -> SteamNewsNormalizer
  -> NewsClassifier
  -> deduplicación dentro del lote Steam
  -> NewsRepository
  -> NewsSyncState
```

El endpoint utilizado es `ISteamNews/GetNewsForApp/v2/`. Se solicita el idioma
`english`, con límites conservadores de cantidad y longitud. No se usan
credenciales Steam para este flujo.

## Límites y errores

El cliente HTTP usa timeout de conexión y petición, user-agent propio,
validación de status HTTP, límite de respuesta de 2 MiB y errores tipados. Los
errores públicos se reducen a códigos no sensibles. Los cuerpos completos de
las noticias no se escriben en logs.

## Normalización y clasificación

Los DTO de Steam permanecen dentro de `news_steam.rs`. El normalizador crea
`NewsItem` canónicos, conserva el contenido original y registra que el idioma
corresponde al idioma solicitado. Steam `feedname` tiene prioridad para la
clasificación; el título solo se usa como señal secundaria cuando el feed no
aporta información. La categoría desconocida termina en `other`.

El contenido con tags HTML se conserva como `originalContent` y se marca como
`html`. No se renderiza, sanitiza destructivamente ni se descargan imágenes en
esta fase.

## Refresco

El refresco es manual mediante comando Tauri. La ventana de frescura es de 15
minutos. Una consulta vigente se omite salvo que `forceRefresh` sea verdadero.
Un fallo conserva el feed anterior, registra `lastAttemptAt`, guarda un código
de error y marca stale cuando existen datos previos.

## Comandos

- `refresh_game_news`
- `get_game_news_feed`
- `get_game_news_sync_state`

No se añadieron componentes React ni hooks de TanStack Query.

## Pruebas

Las pruebas usan fixtures pequeños, proveedores simulados y un servidor HTTP
local. Cubren deserialización, opcionales, idioma, normalización, categorías,
deduplicación por ID y URL, preferencia por contenido completo, HTTP 503,
timeout, persistencia, actualización, errores parciales, stale y política de
freshness.
