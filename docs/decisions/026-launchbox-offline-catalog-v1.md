# LaunchBox Offline Catalog V1

Estado: implementado en código; importación del catálogo real y QA de aplicación
pendientes.

## Responsabilidades

- Eden conserva discovery, Title ID, ruta, lanzamiento, playtime y sesiones.
- LaunchBox local aporta metadata editorial, géneros normalizados, multiplayer
  explícito, rating comunitario y referencias de media.
- SteamGridDB continúa siendo la fuente preferida de cover, hero y logo.
- LumaDeck conserva la identidad canónica, mappings externos, cachés, actividad,
  overrides y presentación.

## Implementación

La migración SQLite 24 crea `launchbox_catalog_state`, `launchbox_games`,
`launchbox_media_refs`, `external_identity_mappings`,
`launchbox_negative_matches` y `launchbox_screenshot_cache`. El catálogo activo
se selecciona por versión y las consultas usan índices por provider ID y
plataforma+título normalizado.

La URL heredada de Ludex era `http://gamesdb.launchbox-app.com/Metadata.zip`.
La implementación usa su equivalente HTTPS:
`https://gamesdb.launchbox-app.com/Metadata.zip`. LaunchBox continúa exponiendo
la base y sus páginas de juegos en `gamesdb.launchbox-app.com`.

El ZIP se descarga a `cache/launchbox/launchbox-metadata.zip.download`, se valida,
se abre y se extrae con la crate Rust `zip` (`default-features = false`,
`deflate`). No se invocan PowerShell, CMD, `tar.exe` ni procesos externos.

La importación es streaming con `quick-xml`: cada `Game` se normaliza y se
inserta por lote dentro de una transacción. `Metadata.xml`/`Metadata.json` son
formatos de importación; SQLite es el formato de runtime. Un fallo revierte la
transacción, elimina temporales y conserva el catálogo activo anterior.

TTL: 30 días. Un catálogo vencido sigue disponible y la actualización se intenta
en background al iniciar. La metadata no bloquea discovery ni lanzamiento.

## Identity y metadata

La prioridad es mapping persistido, título/plataforma exactos, alternate title,
fuzzy de alta confianza y finalmente unresolved. Los matches ambiguos se
rechazan y se almacenan temporalmente en negative cache. El Title ID de Switch
nunca se reemplaza: `external_identity_mappings` guarda la relación
`nintendo_switch + native_id + launchbox + provider_game_id`.

Se almacenan canonical/alternate titles, description, developer, publisher,
release date, raw genres, normalized genres, multiplayer (`true`, `false`,
`unknown`), max local players y rating raw/scale/count. El rating observado en
la fuente de LaunchBox se expresa como estrellas sobre 5; la base conserva el
score y la escala originales, no una conversión a 100.

La evidencia explícita de `Cooperative`, `Multiplayer`, `local multiplayer`,
`play locally` y player count alimenta Local Multiplayer. Un género como
Fighting, por sí solo, no lo activa.

## Media y artwork

Las referencias se clasifican en screenshot, box front/back, clear logo, fanart,
banner y otros. Details descarga como máximo 12 screenshots gameplay en segundo
plano; las imágenes se deduplican por URL/provider ID/hash y se persisten en
`cache/launchbox/screenshots`. Las capturas cacheadas funcionan offline.

El visor existente de Details recibe el mismo `Game.screenshots`; no existe un
viewer específico de LaunchBox. La prioridad visual es user override, selección
cacheada, SteamGridDB, fallback LaunchBox y default.

## Migración Ludex → LumaDeck

| Concepto Ludex                   | Resultado                                          |
| -------------------------------- | -------------------------------------------------- |
| `metadataZipUrl`                 | ADAPTED: HTTPS y descarga temporal                 |
| TTL de 30 días                   | REUSED                                             |
| `Metadata.xml` / `Metadata.json` | REUSED como formatos de importación                |
| `byId` / `byNormalizedName`      | REPLACED por SQLite e índices                      |
| `communityRating`                | ADAPTED: raw score, escala y votos                 |
| image references / screenshots   | ADAPTED a `launchbox_media_refs` y caché on-demand |
| matching                         | REPLACED por identity mapping + confidence         |
| cache version                    | ADAPTED a `catalog_version` y schema version       |
| PowerShell `Expand-Archive`      | REJECTED; reemplazado por crate Rust `zip`         |

## Storage report

El import real no se ejecutó durante esta implementación, por lo que ZIP size,
source size, SQLite size, duración, record count y Switch record count quedan
para la ejecución de QA. Los temporales del flujo sí se eliminan tras éxito o
fallo; el XML fuente no se conserva como cache operativo.

## QA de aplicación

1. Abrir LumaDeck online y esperar `launchbox_catalog_swap_completed`.
2. Ejecutar Refresh Emulator Metadata sin rescanning Eden.
3. Verificar Mario Kart 8 Deluxe y Super Mario Odyssey: Title ID, un solo game,
   mapping persistido, descripción, studio, publisher, fecha, género, rating,
   votos, multiplayer y player count.
4. Abrir Details y confirmar screenshots gameplay, visor fullscreen, retorno de
   foco, artwork overrides y play.
5. Cerrar LumaDeck, desconectar Internet, abrirlo y repetir Library/Details/Play.

El algoritmo de Consensus no mezcla LaunchBox automáticamente en V1; LaunchBox
queda disponible como fuente community separada de Metacritic/OpenCritic.
