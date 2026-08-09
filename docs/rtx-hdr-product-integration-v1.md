# RTX HDR product integration V1

## Scope

LumaDeck applies RTX HDR only to the per-game NVIDIA profile mapped to the
game executable. It does not automate NVIDIA App, Overlay, Game Filters,
RHI.exe, Profile Inspector, or proxy DLLs.

The raw NVAPI path is the one validated by the isolated spike:

- `NvAPI_Initialize` and the DRS session lifecycle;
- `NvAPI_DRS_FindApplicationByName` with the canonical executable path;
- raw `GetSetting` / `SetSetting`, `SaveSettings`, and
  `DeleteProfileSetting`;
- RTX HDR IDs `0x00DD48FB` through `0x00DD48FF` and Debanding
  `0x00432F84`.

Natural and Vibrant currently share the proven V1 values. Peak brightness is
stored on the game display profile and defaults to 800 nits.

## Policy

`AUTO` resolves in this order:

1. Native HDR = YES and HDR display support = YES: native HDR.
2. Native HDR = NO and compatible NVIDIA hardware: RTX HDR Natural.
3. Native HDR = UNKNOWN: no automatic HDR action.
4. Otherwise: the existing conservative fallback.

Native HDR never writes RTX settings. SYSTEM returns before reading or
writing Windows HDR, Auto HDR, or NVIDIA settings.

## Launch and recovery

Before launch the service captures the Windows display state, the exact six-
setting RTX snapshot, and the per-executable Auto HDR registry value. The
journal is written before any mutation. RTX profiles require Windows HDR to
be enabled, Auto HDR to be disabled for that executable, and a verified raw
NVAPI readback in a new session. Any explicit RTX failure rolls back and
blocks launch.

On exit or startup recovery, RTX settings are restored as explicit raw values
or deleted when the snapshot was inherited/missing. Auto HDR and Windows HDR
are then restored. The journal is cleared only after all readbacks succeed.

## Prerequisites and QA boundary

The UI exposes System, Automatic, Native HDR, RTX HDR Natural, RTX HDR
Vibrant, and Disabled per game. RTX options are hidden unless compatible
NVIDIA hardware is detected. NVIDIA App presence is reported by the backend;
Overlay and Game Filters remain a manual prerequisite and are never opened or
automated by LumaDeck.

The product tests cover raw-value encoding, exact snapshot serialization,
inherited/explicit restore policy, AUTO resolution, native/system policies,
and the journal rollback path. JEDI Fallen Order and Marvel Tōkon remain the
manual hardware QA targets for overlay and television cross-checks.
