# 017 — News domain and storage V1

## Estado

`NEWS DOMAIN AND STORAGE V1: IMPLEMENTED`

Steam provider, translation provider, UI de Noticias y QA de aplicación real
permanecen pendientes.

## Alcance implementado

La primera fase implementa únicamente el dominio canónico y la persistencia
SQLite. No contiene HTTP, Steam Web API, proveedores de traducción, comandos
Tauri, React, TanStack Query, sincronización en segundo plano, Hero ni
agregación entre proveedores.

El módulo Rust `news` contiene:

- `NewsItem`, `NewsTranslation` y `NewsSyncState`.
- `NewsCategory`, `NewsContentFormat` y `TranslationStatus`.
- Identidad estable separada del hash de contenido.
- Hash SHA-256 determinista del idioma, título, resumen y contenido originales.
- `NewsRepository` como única frontera de persistencia para noticias,
  traducciones y estado de sincronización.

## Persistencia

La migración SQLite 13 crea:

- `news_items`.
- `news_translations`.
- `news_sync_state`.

La migración es idempotente y mantiene las claves foráneas activas. Los
índices se limitan a consultas por juego/fecha, categoría, proveedor/URL
canónica, traducción reutilizable y estado de sincronización.

## Reglas de actualización

El upsert busca primero por `provider_id + external_id` y después por
`provider_id + canonical_url`. Si encuentra una noticia, conserva su ID,
actualiza los datos y marca stale únicamente las traducciones cuyo hash de
origen ya no coincide. Las traducciones históricas no se eliminan.

Una traducción reutilizable debe tener el mismo hash de contenido original,
idioma destino y estado `translated`. Una traducción `failed` se almacena como
estado independiente y nunca invalida ni elimina el contenido original.

## Validación

Las pruebas Rust cubren inserción, upsert sin duplicados, orden descendente,
filtros, hash estable, reutilización e invalidación de traducciones, fallback
ante fallo, estado de sincronización y migración idempotente.
