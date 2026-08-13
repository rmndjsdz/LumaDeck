# PCGamingWiki Provider V1.2

## Decisión de arquitectura

La resolución runtime de identidad usa exclusivamente la API MediaWiki de
PCGamingWiki (`/w/api.php`). Los endpoints personalizados `api/appid.php` y
`api/gog.php`, las páginas HTML y cualquier mecanismo de evasión de Cloudflare
son diagnósticos/documentación, no dependencias del pipeline.

El pipeline es:

`Steam App ID | GOG Product ID → MediaWiki CargoQuery → pageId + pageTitle → MediaWiki action=parse → Template Video → capability evidence → SQLite cache`

El parser de `Template Video` se conserva. React solo recibe el resultado
tipado de `pcgamingwikiService`.

## Schema Cargo real

La instalación actual expone el schema mediante:

```text
action=cargotables
action=cargofields&table=Infobox_game
```

El mapping usa la tabla `Infobox_game` y estos campos:

| Propósito         | Campo Cargo   | Formato observado                  |
| ----------------- | ------------- | ---------------------------------- |
| Steam             | `Steam_AppID` | `String`, lista separada por comas |
| GOG               | `GOGcom_ID`   | `String`, lista separada por comas |
| Título            | `_pageName`   | referencia de página               |
| Identidad estable | `_pageID`     | page ID como string                |

La nomenclatura corresponde a la [plantilla oficial Infobox game](https://www.pcgamingwiki.com/wiki/Template%3AInfobox_game)
y las llamadas se realizan contra la [API de PCGamingWiki](https://www.pcgamingwiki.com/wiki/PCGamingWiki%3AAPI).

## Queries de identidad

Steam usa una query exacta y determinista:

```text
action=cargoquery
tables=Infobox_game
fields=Infobox_game._pageName=Page,Infobox_game._pageID=PageID,Infobox_game.Steam_AppID,Infobox_game.GOGcom_ID
where=Infobox_game.Steam_AppID HOLDS "3787240"
limit=1
```

Devuelve `Marvel Tōkon: Fighting Souls`, `PageID=205571` y un valor de Steam
que contiene el ID consultado. La relación no está hardcodeada.

GOG usa la query equivalente:

```text
where=Infobox_game.GOGcom_ID HOLDS "1785384169"
limit=1
```

En la QA real actual, el predicado exacto devuelve directamente `Carrion`,
`PageID=139686`, `GOGcom_ID=1785384169,1086484900`. El proveedor conserva
además un fallback determinista que sigue siendo Cargo para instalaciones
donde ese predicado no indexe correctamente: consulta `GOGcom_ID HOLDS LIKE
"%"` en páginas de 500 filas y compara localmente los tokens separados por
comas.

No hay búsqueda por título, fuzzy matching ni uso de `api/gog.php`.

## Identidad y persistencia

El contrato runtime mínimo es:

```text
PCGamingWikiGameRef {
  pageId,
  pageTitle,
  steamAppId?,
  gogProductId?,
  resolvedVia
}
```

`pageId` es la identidad estable preferida. La columna SQLite histórica
`page_identifier` conserva ese valor por compatibilidad; la migración 28 añade
`identity_checked_at`.

Cuando ambos IDs se solicitan, Steam es primario. El chequeo opcional de GOG
compara identidades por `pageId` y expone `PCGW_IDENTITY_CONFLICT` si difieren.

## Capabilities

Después de resolver Cargo, el proveedor llama a:

```text
action=parse&page=<pageTitle>&redirects=1&prop=wikitext&format=json&formatversion=2
```

El parser extrae `{{Video}}` para `hdr`, `upscaling` y sus tecnologias,
`framegen` y sus tecnologias, `4k ultra hd`, `60 fps`, `120 fps` y la
nota de `120 fps`. Estos campos son capacidades del juego; no se usan para
estimar rendimiento ni para sustituir el modo actual del display. La ausencia
de un valor permanece como `UNKNOWN`.

Una nota como `Capped to 60 FPS.` se conserva como evidencia de
`HIGH_REFRESH_120_FPS`; no se convierte en una estimacion de FPS ni en la
frecuencia actual del display. Una alternativa o workaround nunca cambia el
estado de `NATIVE_HDR`: `NO + alternativeAvailable=YES` sigue siendo HDR
nativo no compatible.

## Cache y errores

- Identidad: TTL de 30 días.
- Capabilities: TTL de 7 días.
- ETag/Last-Modified se conservan para `action=parse` cuando están disponibles.
- Requests concurrentes se coalescen por identidad.
- Un cache fresco evita HTTP; un fallo de refresh sirve cache previo con
  `stale=true`.
- `NOT_FOUND`, API malformada/no disponible, timeout, rate limit y
  `PCGW_FORBIDDEN` permanecen diferenciados. Un 403 de los endpoints
  personalizados no participa en el pipeline normal.
- No se persiste HTML completo; se persiste mapping, evidencia versionada y
  metadatos de cache.

## QA real V1.2

La prueba ignorada `real_mediawiki_identity_and_capabilities_qa` ejecuta el
pipeline con el cliente `reqwest` de LumaDeck y sin navegador:

```text
Steam 3787240 → CargoQuery → Marvel Tōkon: Fighting Souls / 205571
             → action=parse → HDR / Upscaling / FrameGen

GOG 1785384169 → CargoQuery → Carrion / 139686
              → action=parse → HDR / Upscaling / FrameGen
```

Se ejecuta con:

```text
cargo test pcgamingwiki::tests::real_mediawiki_identity_and_capabilities_qa --lib -- --ignored --nocapture
```

Resultado observado en la ejecución QA del 9 de agosto de 2026:

- Marvel Tōkon: `HDR=NO/HIGH`; `upscaling=YES/HIGH` con `TSR`, `DLSS 4`,
  `NIS`, `FSR 4`, `XeSS 2`; `framegen=NO/HIGH`.
- Carrion: `HDR=NO/HIGH`; `upscaling=UNKNOWN/LOW`; `framegen=UNKNOWN/LOW`.
- Cold cache: 4 requests totales, dos por juego (`cargoquery` + `parse`).
- Warm cache: 0 requests adicionales para abrir Details repetidamente.

## Limitaciones

- El fallback GOG está limitado a 5.000 filas Cargo; superar ese límite se
  reporta como fallo temporal, no como `NOT_FOUND`.
- Algunas instalaciones Cargo pueden no aplicar el predicado exacto GOG a
  listas; el fallback paginado cubre ese caso sin salir de MediaWiki.
- Artículos sin datos `Video` producen evidencia independiente `UNKNOWN`.
- No se implementan Epic, emuladores, fuzzy matching, lanzamiento, activación
  HDR ni detección de GPU.
