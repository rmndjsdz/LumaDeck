# SteamGridDB artwork enrichment V1

The Settings → Integrations → SteamGridDB screen starts an
ArtworkEnrichmentService run. It reuses the existing SteamGridDbClient,
artwork downloader, image validator, cache and manual artwork persistence.

V1 is fill-missing only. The five supported slots are horizontal grid,
vertical grid, square grid, hero and logo. Candidate selection first validates
the slot aspect ratio, then prefers native pixel density, provider score and
community votes. A logo with equal density prefers PNG so alpha is retained.

Images are decoded, validated, optionally downscaled to the configured longest
side (4096px by default), encoded as lossless WebP with RGBA, hashed and
atomically committed. Downscale is never used to upscale. The content hash is
the physical cache identity, so identical assets are deduplicated.

Manual selections are stored as steamgriddb_manual with user_locked = 1.
Automatic selections use steamgriddb_auto with user_locked = 0. Existing
fallbacks and legacy selections are treated conservatively as protected.

Negative cache entries are provider/slot/game scoped and expire after seven
days. Runs use a bounded worker pool (default four, maximum eight), retry
transient offline/timeout/rate-limit failures with backoff, and persist a
summary. Cancellation stops new work and leaves committed assets intact;
reruns inspect selections and cache before downloading.

The current V1 does not expose refresh-automatic-artwork mode or a separate
pending-review editor. Ambiguous matches are recorded as a partial result and
remain available to the existing manual SteamGridDB flow.
