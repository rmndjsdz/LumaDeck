# RTX HDR real integration spike V2

Status: `RTX_HDR_DRS_CONFIRMED` for the installed NVIDIA profile path, with manual overlay/runtime QA still requiring a human-visible cross-check.

This is an isolated experiment. It does not alter `GraphicsProfileResolver`, `LaunchDisplayOrchestrator`, capabilities, UI, DB, or game launch code. It never targets `_GLOBAL_DRIVER_PROFILE`; the lookup is the fully-qualified Jedi executable and the returned application profile.

## RHI implementation found

Source checked from `RankFTW/RHI`, commit `b5ff5bba2a604b10fdbc0b4200c60c3af2b9e066`, RHI 2.3.1 source snapshot.

- `RenoDXCommander/Services/DlssPresetService.cs`: raw NVAPI function lookup, setting buffer construction, `SetSettingRawNvApi`, `GetSettingRawNvApi`, `DeleteSettingRawNvApi`.
- `RenoDXCommander/Services/DlssPresetService.DriverSettings.cs`: RTX HDR IDs and `SetRtxHdrRaw`.
- `RenoDXCommander/Services/DlssPresetService.ProfileMatching.cs`: application/profile lookup and the optional profile/application creation path.
- `RenoDXCommander/MainWindow.Events.Components.cs`: user-facing conversions and the six RTX HDR writes.

The raw path resolves `nvapi_QueryInterface` from `nvapi64.dll` and uses:

| Operation        | Interface ID | Signature used by RHI                    |
| ---------------- | -----------: | ---------------------------------------- |
| Initialize       | `0x0150E828` | `NvAPI_Initialize()`                     |
| Create session   | `0x0694D52E` | `NvAPI_DRS_CreateSession(hSession*)`     |
| Load settings    | `0x375DBD6B` | `NvAPI_DRS_LoadSettings(hSession)`       |
| Find application | `0xEEE566B2` | `NvAPI_DRS_FindApplicationByName(...)`   |
| Get setting      | `0xEA99498D` | raw 5-argument form with `extraParam`    |
| Set setting      | `0x8A2CF5F5` | raw 5-argument form with trailing `0, 0` |
| Save settings    | `0xFCBC7E14` | `NvAPI_DRS_SaveSettings(hSession)`       |
| Delete setting   | `0xE4A26362` | `NvAPI_DRS_DeleteProfileSetting(...)`    |

RHI allocates exactly 12,320 bytes for `NVDRS_SETTING_V1` and writes version `size | (1 << 16)` (`0x00013020`). The DWORD `currentValue` is at byte offset `8,220`. The spike reproduces this raw mechanism without `NvAPIWrapper`.

RHI first resolves an application/profile mapping, then writes against that returned profile handle. Its optional creation path creates a named profile and registers an executable; the spike deliberately does not create a profile automatically because Jedi already has a real NVIDIA mapping and the mutation test must remain narrowly scoped.

## Setting IDs and encoding

These are the IDs and encodings declared/used by RHI. `Default` below means the RHI/NVIDIA effective value; the readback command reports the driver-provided predefined value instead of treating a missing per-game override as an explicit default.

| Setting         |           ID | Type  |             Off value |     On value | RHI default / encoding                     |
| --------------- | -----------: | ----- | --------------------: | -----------: | ------------------------------------------ |
| RTX HDR Enable  | `0x00DD48FB` | DWORD |          `0x00000000` | `0x00000001` | off/on DWORD                               |
| Peak Brightness | `0x00DD48FC` | DWORD | `0` / delete override |          n/a | 400–2000 nits, raw DWORD                   |
| Middle Grey     | `0x00DD48FD` | DWORD |                   n/a |          n/a | 10–100, default 50, raw DWORD              |
| Contrast        | `0x00DD48FE` | DWORD |                   n/a |          n/a | stored `100 + display`, display −100..+100 |
| Saturation      | `0x00DD48FF` | DWORD |                   n/a |          n/a | stored `100 + display`, display −100..+100 |
| Debanding       | `0x00432F84` | DWORD | `0x06` (No Debanding) |          n/a | `0x06`, `0x0A`, `0x02`, `0x03`, `0x23`     |

RHI also writes `0x1077A11A` (`RTX HDR Allow`, `0x00` disallow / `0x01` allow) while enabling from its UI. It is intentionally outside this six-setting spike table and is not mutated by the test commands.

## Why the old spike failed

The old `0x00DD48FB` result (`NvAPI_DRS_GetSetting`/`SetSetting` → `NVAPI_SETTING_NOT_FOUND`, `-160`) is not evidence that the driver lacks RTX HDR. RHI bypasses the wrapper and resolves the newer raw interfaces `0xEA99498D` and `0x8A2CF5F5`, uses the exact 12,320-byte V1 layout/version, passes the live per-application profile handle, and calls `SaveSettings` explicitly. The public/legacy wrapper path used by the old spike did not reproduce all of those conditions.

## Commands

Run from the repository root with the game closed:

```powershell
cargo run --manifest-path tools/rtx-hdr-spike/Cargo.toml -- get
cargo run --manifest-path tools/rtx-hdr-spike/Cargo.toml -- set-contrast 20
cargo run --manifest-path tools/rtx-hdr-spike/Cargo.toml -- restore-contrast
cargo run --manifest-path tools/rtx-hdr-spike/Cargo.toml -- toggle-off
cargo run --manifest-path tools/rtx-hdr-spike/Cargo.toml -- toggle-restore
```

The first command is GET-only. Mutation commands snapshot whether each setting existed, its raw value, type, location, predefined value, and application/profile mapping. If a setting was inherited, restore deletes the per-game override rather than writing an equivalent value.

## Machine result

Target resolved from the installed game, not Steam AppID:

`F:\SteamLibrary\steamapps\common\Jedi Fallen Order\SwGame\Binaries\Win64\starwarsjedifallenorder.exe`

Raw profile lookup returned:

`Star Wars Jedi: Fallen Order` → `starwarsjedifallenorder.exe` → per-game profile handle.

GET-only, before mutation:

| Setting         | Raw value |  Display | Location        | Explicit override    |
| --------------- | --------: | -------: | --------------- | -------------------- |
| RTX HDR Enable  |       `1` |       ON | CURRENT_PROFILE | yes                  |
| Peak Brightness |     `800` | 800 nits | CURRENT_PROFILE | yes                  |
| Middle Grey     |      `60` |       60 | CURRENT_PROFILE | yes                  |
| Contrast        |     `115` |      +15 | CURRENT_PROFILE | yes                  |
| Saturation      |     `100` |        0 | CURRENT_PROFILE | yes                  |
| Debanding       |         — |        — | —               | no; absent/inherited |

The driver reported predefined DWORD `0` for the five present RTX HDR settings. The values match the manually reported Contrast +15, Middle Grey 60, Saturation 0 and RTX HDR ON; the live profile readback is Peak Brightness 800 nits (not the approximate 700 nits in the brief).

SET contrast:

`115 (+15) → 120 (+20)` via raw `NvAPI_DRS_SetSetting(0x8A2CF5F5)` followed by `NvAPI_DRS_SaveSettings(0xFCBC7E14)`. A new NVAPI session read back raw `120` / display `+20`.

RESTORE contrast:

The exact snapshot restored raw `115` / display `+15`; a new NVAPI session read back the original value.

TOGGLE:

`RTX HDR Enable 1 (ON) → 0 (OFF)`; a new session read back `0` / OFF. Snapshot restore returned `1` / ON in a new session. Debanding was absent in the snapshot and is intentionally left absent rather than forcing a delete that some drivers reject for an already-missing newer ID.

NVIDIA App / Overlay prerequisite check: NVIDIA App 11.0.8.299 is installed and the NVIDIA App directory exists. This read-only check cannot prove the per-user Overlay/Game Filters toggles without UI inspection; the brief's prior manual QA already established that Game Filters was enabled for the successful physical test.

Admin requirement: all GET, SET, SAVE, readback, and RESTORE operations succeeded as a normal process. No elevation, helper, or permanent admin mode was used.

NVIDIA Overlay cross-check: not automatable from this spike; manually confirm Contrast +20 while the temporary test is active, then confirm +15 after restore. The driver readback itself confirmed both values.

Runtime Jedi QA: after restore, the installed executable was launched and remained active as `starwarsjedifallenorder.exe` (PID 11748) after the launcher transition. No game process was running during any profile mutation. Windows HDR ON / Auto HDR OFF and the NVIDIA Overlay RTX HDR ACTIVE / HDR10-PQ checks remain manual visual checks; the spike does not automate or alter those preferences.

## License

RHI is GPL-3.0-only (`RankFTW/RHI/LICENSE`). This spike reproduces the raw API contract and does not copy RHI source or use RHI as an executable/dependency. NVIDIA NVAPI headers are public SDK material; only the required function IDs/layout constants are reproduced here. A future distributable integration needs a separate license review, especially for GPL compatibility.
