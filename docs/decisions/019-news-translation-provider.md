# ADR 019 — News translation provider V1

## Decision

LumaDeck translates persisted `NewsItem` titles and optional summaries on
demand through a backend-only `TranslationProvider` contract. The original
title, summary and full content remain persisted and are always the fallback.
Full article content is intentionally never sent to the provider.

The first provider is Google Cloud Translation Basic v2, using the official
REST endpoint:

`POST https://translation.googleapis.com/language/translate/v2`

The provider version recorded in the cache is `v2-basic-nmt`. Google documents
API-key authentication for Basic v2 and a maximum of 128 `q` strings per
request. LumaDeck batches at most 64 news items and applies a conservative
30,000-character batch limit. The exact product target `es-419` is supported
by the current Google language list, so no conversion to `es-SV` is made.

## Credentials and distribution boundary

The API key is accepted only by Tauri commands, encrypted with the existing
Windows DPAPI current-user mechanism, and never returned to React, logged,
serialized into public assets, or included in error messages. This is suitable
for local development and controlled local use only. A public distributed
build must move the provider call behind a LumaDeck backend or use an
explicit per-user credential flow; this ADR does not add that remote backend.

## Cache and failure policy

The existing `news_translations` table is reused. A cache identity includes
the news item, source/target language, source content hash, provider/version
and glossary version. A changed source hash creates a new translation identity
and marks the previous row stale, preserving history. `force_retranslate`
skips a valid cache row. Partial results and typed provider failures are
persisted without exposing provider response bodies or credentials.

Google Basic v2 has no glossary integration in this implementation. A
`glossaryVersion` may still participate in the cache key, but no unsafe local
term substitution is performed.
