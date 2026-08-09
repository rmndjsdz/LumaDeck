# Eden Discovery V1

## Scope

LumaDeck accepts one required input: the selected `eden.exe`. Discovery is read-only against Eden configuration, ROMs and saves. It imports detected games into the existing `games` / `game_provider_links` model with `source = emulator`, `provider = Eden`, `platform = Nintendo Switch`, `emulator_id = eden`, `game_path` and optional `title_id`.

## Current Eden paths and format

The current Eden documentation describes these Windows data locations:

- Normal: `%APPDATA%\eden\config\qt-config.ini`; data root is `%APPDATA%\eden`.
- Portable: a `user` directory beside `eden.exe`, with `user\config\qt-config.ini`; data root is the portable `user` directory.
- Game directories: QSettings array `gamedirs`, using either sectioned keys under `[Paths]` (`gamedirs\N\path`) or the flattened form emitted by the current Windows build (`Paths\gamedirs\N\path` and `Paths\gamedirs\N\deep_scan`). The parser also accepts the legacy `gameListRootDir` / `gameListDeepScan` pair for older compatible configurations. Eden's virtual roots (`SDMC`, `UserNAND`, and `SysNAND`) are excluded from filesystem scanning.
- Local profiles: `nand\system\save\8000000000000010\su\avators\profiles.dat`; the active index is read from `current_user` in `qt-config.ini` and avatar images are read from the neighboring UUID-named `.jpg` file. LumaDeck displays this information read-only; profile creation, switching, and deletion remain owned by Eden.

LumaDeck prefers a portable `user` directory when it exists, then falls back to `%APPDATA%\eden`. It never writes `qt-config.ini`. If the configuration is missing/corrupt, the user can add manual roots; those roots are stored only in LumaDeck's own database.

## V1 game discovery and identity

Only `.nsp` and `.xci` are scanned, case-insensitively, within configured roots. `deep_scan` controls recursive traversal. A disconnected/inaccessible root is reported and does not mark existing games below that root as missing. A successfully scanned root can mark absent previously imported games as `installed = false` with `missing_since`, without deleting their record.

Title ID resolution is ordered as follows:

1. A valid `010` + 13 hexadecimal ID in the game path/name.
2. A name-to-ID mapping read from `log\eden_log.txt` or `eden_log.txt.old.txt` when Eden has emitted `Loading ... (TITLE_ID)` or `Booting game: TITLE_ID | NAME`.
3. Unavailable. No ROM bytes are modified or parsed in V1.

`play_time\playtime.bin` is read as 16-byte records (`u64` little-endian Title ID, `u64` little-endian seconds). `cache\launched.json` is optional and supplies `timestamp` by Title ID. Statistics never gate discovery.

Games with the same Title ID are deduplicated. Games without a Title ID use a stable hash of the normalized path. This is a discovery identity, not external metadata matching.

## Eden Game Identity Reconciliation V1

The persisted identity is installation-scoped. Once available, the canonical
key is `source + emulatorInstallationId + titleId`, represented by a
deterministic LumaDeck `game_id`. A normalized path is only a provisional
identity and is never the final identity for a game with a Title ID.

During a rescan, the canonical row is created or updated inside the same
transaction that reconciles provisional and historical rows. References to
sessions, activity, metadata, artwork selections, provider links, external
playtime snapshots and other `game_id` foreign keys are moved before the
provisional row is deleted. Favorites, hidden state, progress, playtime and
the most recent `last_played_at` are preserved. Eden external playtime keeps
the highest observed snapshot and does not add the provisional and canonical
totals together.

If multiple physical files expose the same Title ID in one scan, the
lexicographically smallest normalized path is selected as the active path.
Other files are not deleted. A later path change updates the canonical row and
does not create another Library game.

Existing Eden rows with a Title ID are normalized to the installation-scoped
canonical ID during every successful connect/rescan, so historical duplicates
are cleaned without waiting for a future identity transition. The operation
is idempotent and is rolled back with the outer transaction if any reference
migration fails.

Identity diagnostics use the checkpoints
`eden_identity_provisional_created`, `eden_title_id_discovered`,
`eden_identity_reconciliation_started`,
`eden_identity_reconciliation_completed`,
`eden_duplicate_title_id_detected` and `eden_game_path_updated`. Details
contain only `gameId`, `titleId` and `emulatorInstallationId`; full filesystem
paths are excluded.

## Launch and deferred work

Basic launch uses the current Eden CLI contract `eden.exe -g <game-path>`. Session/process handoff tracking is intentionally deferred. DLC, updates, saves, artwork and external metadata remain outside this slice.

## LUDEX REUSE REPORT

### REUSE

- Eden's supported `.nsp` / `.xci` baseline.
- Title ID shape and the safe `playtime.bin` / `launched.json` readers.
- `eden_log.txt` loading/booting patterns as an optional name-to-ID source.
- The distinction between an emulator executable and the imported game path.

### ADAPT

- `Emulator Profiles`: reduced to an `Eden` definition plus persisted installation data; no speculative registry for every emulator.
- `scanEmulatorGames`: rewritten around configured roots, `deep_scan`, root availability and stable identity rather than fixed depth/file limits.
- Eden data discovery: adds the documented portable `user` root and prefers it over normal AppData when present.
- `switchTitleId`: kept as optional strong identity, but path/name fallback is explicit and does not pretend to be a Title ID.

### REJECT

- Filename-only identity as the primary model.
- Fixed scan depth and arbitrary 600-file cap.
- LaunchBox lookups, external metadata and provider-specific visual libraries inside discovery.
- Complex Eden process handoff/session tracking from Ludex.

### DEFER

- `playtime.bin`, `launched.json` and log statistics are read opportunistically; richer reconciliation is deferred.
- Saves/cloud backup, process tracking, DLC/update association and LaunchBox metadata.
- Cemu, Dolphin, RPCS3 and a generalized emulator registry.
