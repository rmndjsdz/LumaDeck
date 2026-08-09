# Hardware Capabilities V1

Hardware Capabilities V1 uses Windows DXGI as the read-only source of truth.
`IDXGIFactory1::EnumAdapters1` enumerates all adapters and
`IDXGIAdapter1::GetDesc1` supplies the vendor ID, device ID, description,
dedicated video memory, software flag, and adapter LUID.

The normalized model contains `GpuAdapter[]`, a deterministic
`preferredGamingGpu`, vendor/model/VRAM/architecture, driver version when a
reliable source is available, tri-state feature support, confidence, and a
diagnostic. Microsoft Basic Render Driver and DXGI software adapters are never
eligible as the preferred gaming GPU.

## Preferred GPU policy

1. Exclude software adapters.
2. Prefer a non-software hardware adapter with known vendor.
3. Use dedicated VRAM as the deterministic fallback when Windows does not
   expose a high-performance preference.
4. Keep every adapter in the snapshot; do not assume the first enumerated
   adapter is the gaming GPU.

## Feature normalization

- NVIDIA GTX: DLSS and DLSS Frame Generation unsupported.
- RTX 20/30: DLSS supported, DLSS Frame Generation unsupported.
- RTX 40: DLSS and DLSS Frame Generation supported.
- RTX 50: DLSS supported and DLSS Frame Generation unknown until a verified
  family rule is added.
- FSR is modeled as broadly usable on real hardware, not AMD-only.
- XeSS is supported on Intel Arc, while Intel integrated adapters remain
  unknown; `preferredXess` is only asserted for Arc. Other vendors remain
  conservative/unknown.
- No `DLSS 4` hardware claim is made; game technology version and hardware
  family are kept separate and crossed only by Graphics Profile Resolver.

## Cache and failure behavior

The first read caches one hardware snapshot for the process. Explicit refresh
re-enumerates DXGI. Hardware is not persisted as permanent SQLite truth and is
not re-enumerated during focus/render changes. If DXGI fails, the service
returns an UNKNOWN snapshot with a diagnostic so Details and the resolver stay
usable.

No HTTP, PowerShell, WMI, driver changes, elevation, registry writes, or
display-setting changes are used.
