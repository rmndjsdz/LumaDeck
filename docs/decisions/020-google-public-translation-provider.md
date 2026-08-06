# ADR 020 — Google Public Translation provider V1

## Decision

LumaDeck adds `GooglePublicTranslationProvider` as a second implementation of
the existing `TranslationProvider` contract. Its identity is `google-public`
and `gtx-v1`. It translates only persisted news titles and optional summaries,
uses the existing `TranslationService`, and reuses the existing SQLite cache.

The transport is a backend-only POST to:

`https://translate.googleapis.com/translate_a/single`

with `client=gtx`, `sl`, `tl`, `dt=t` and form-body `q`. It is a public,
unofficial endpoint, not Google Cloud Translation, and has no contractual SLA.

## Risk boundary

This provider may change without notice, rate-limit with HTTP 429, block
automated traffic, return a captcha or HTML response, or stop working. It is
intended for development, personal use, functional tests and low-volume
best-effort translation. It must not be treated as the only dependency of a
public commercial distribution.

The provider requires no credential, never reads the Google Cloud API key,
and never accepts an endpoint from React. Google Cloud Translation remains the
official configurable alternative and retains its DPAPI credential flow.

## Language, limits and parsing

The domain keeps the canonical target `es-419`; the public transport explicitly
sends `tl=es`. Unknown source languages use `sl=auto`; trusted English sends
`sl=en`. Requests are limited to eight items and 4,000 characters per batch,
with 1,000-character fragments. Fragment boundaries prefer paragraphs, then
sentences, then a hard character boundary. Responses are capped at 512 KiB and
parsed defensively as the known nested array shape. Empty, invalid, truncated,
HTML and captcha-like responses are failures and never become successful
translations.

At most one small retry is attempted for transient network/5xx failures;
HTTP 429 is returned as `rate_limited` without an aggressive retry. The
original news fields remain the fallback for every failure.

## Selection and cache

With no explicit selection, `google-public` is active. Selecting
`google-cloud-translation` is explicit and does not silently fall back to the
public provider or retry through a paid provider. A public-provider failure
therefore remains a failure with original-content fallback.

Provider ID and version are part of the translation cache key, so public and
Cloud translations remain separate and historical rows are not overwritten.
No glossary support is claimed and `supportsGlossary` is false.

## Future distribution

Before public distribution, prefer a LumaDeck backend or an explicit per-user
credential/service design. The unofficial endpoint should remain an opt-in
best-effort development/personal capability unless its usage constraints are
revalidated.
