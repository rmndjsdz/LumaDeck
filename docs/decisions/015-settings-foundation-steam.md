# 015 — Settings foundation and Steam credentials

## Decision

LumaDeck stores persistent settings in one SQLite database named `lumadeck.db`.
The database is created below the Tauri application data directory. The
frontend never opens SQLite directly; it calls narrow Tauri commands through
`ProviderSettingsService`.

The first migration creates separate tables for `providers`,
`provider_accounts`, `provider_credentials`, `app_settings`, and
`schema_migrations`. The Steam provider is seeded with `INSERT OR IGNORE` so
reopening the app is idempotent. Account and credential writes use a single
transaction, with a unique `(provider_account_id, credential_type)` key and
foreign keys enabled.

## Credential protection

The Steam Web API Key reaches Rust only through the save commands. On Windows
it is protected with DPAPI `CurrentUser`, stored as a BLOB, and never returned
to the frontend. The frontend receives only a configured flag and a masked
suffix. A BLOB protected for another Windows user, machine, or copied database
is reported as `credential-unavailable`; SteamID64 and the rest of the
database remain usable until the user replaces the key.

Non-Windows builds keep the same command and schema contracts but report
credential protection as unavailable. This keeps pure validation and database
tests portable while Windows-specific DPAPI tests can run on Windows CI.

## Ownership

- React owns presentation, navigation level, and short-lived input drafts.
- `ProviderSettingsService` owns frontend command DTOs and safe error mapping.
- Tauri commands own the public backend API and never accept SQL or arbitrary
  blobs.
- Rust repositories own account/credential persistence and transaction scope.
- DPAPI owns protection and unprotection in backend memory only.
- Zustand navigation/product stores do not persist settings or credentials.

## Scope of V1

Settings, Integrations, and Steam account configuration are navigable with the
existing Screen Adapter and focus engine. Epic, Xbox, PlayStation, Ubisoft,
and GOG are declarative disabled provider rows. No Steam network call,
library import, sync history, or online credential verification is included.
