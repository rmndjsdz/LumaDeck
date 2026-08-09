# Windows HDR V1

## Alcance

LumaDeck controla exclusivamente el estado HDR del sistema operativo Windows
(Advanced Color) por pantalla. Esta capa no implementa RTX HDR, Auto HDR,
perfiles por juego, metadatos HDR10, Dolby Vision ni comandos del televisor.

## API de Windows utilizada

El backend Tauri usa la API oficial CCD/Display Configuration:

- `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` enumera las rutas activas.
- `DisplayConfigGetDeviceInfo` con
  `DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO` consulta `advancedColorSupported`
  y `advancedColorEnabled`.
- `DisplayConfigSetDeviceInfo` con
  `DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE` solicita activar o
  desactivar Advanced Color.

La estructura consultada conserva también el encoding y los bits por canal,
pero V1 no los expone en la UI. Windows sigue siendo la fuente de verdad:
después de solicitar el cambio se vuelve a consultar el estado durante una
ventana acotada de reconciliación y sólo se informa éxito cuando el estado
efectivo coincide.

## Identidad y multi-monitor

El ID público es el nombre source GDI que ya utilizaba DisplayService
(`\\.\DISPLAY1`, etc.). Para HDR, ese source se resuelve en la ruta activa y
se utilizan el `adapterId` y `targetInfo.id` del target correspondiente. Esto
evita identificar la pantalla por nombre, fabricante, resolución o EDID
interpretado superficialmente. Cada llamada recibe explícitamente `displayId`;
no existe un estado HDR global.

La enumeración devuelve `hdrSupported`, `hdrEnabled` y `hdrStatus` por display.
`hdrStatus` puede ser `supported`, `unsupported` o `unknown`. Un fallo de la
API es `unknown`, no una conclusión de que el monitor sea SDR.

## API de DisplayService

```text
getHdrState(displayId)
setHdrEnabled(displayId, enabled)
captureHdrState(displayId)
restoreHdrState(snapshot)
captureDisplaySnapshot(displayId)
restoreDisplaySnapshot(snapshot)
```

`HdrSnapshot` sólo contiene `displayId` y `enabled`, por lo que restaurar HDR
no cambia resolución, frecuencia ni escala. `DisplaySnapshot` contiene modo,
escala y snapshot HDR para el flujo futuro de perfiles de juego; su restauración
aplica cada dimensión por separado y vuelve a capturar el estado real al final.

## Hotplug y cambios externos

La vista vuelve a enumerar displays cada cinco segundos. Ese polling es el
fallback actual para hotplug y cambios hechos desde Windows Settings o
`Win + Alt + B`; no se simula el atajo ni se abre Settings. Si una pantalla
desaparece, el siguiente ciclo elimina su estado y la selección cae en un
display aún conectado. Un error de operación se muestra como mensaje seguro,
manteniendo el código técnico sólo en el error/log del backend.

## Validación

Validado automáticamente en este workspace:

- `npm run typecheck`
- `cargo fmt --check`
- `cargo check`
- pruebas unitarias del dominio HDR (SDR, HDR apagado/encendido, desconocido,
  multi-monitor, reconciliación, rechazo y snapshot HDR-only)

La validación física con Windows 11 y un monitor/TV HDR requiere hardware HDR
conectado y debe ejecutarse manualmente: activar/desactivar desde LumaDeck,
usar `Win + Alt + B`, cambiar modo con HDR activo y comprobar la señal del TV.
No se declara realizada esa parte sin el dispositivo conectado.

## Limitaciones conocidas

- No hay todavía una notificación nativa de cambio de Advanced Color integrada
  en la UI; el polling de cinco segundos evita polling agresivo y cubre el
  fallback de hotplug/cambios externos.
- La operación nativa puede producir blackout o renegociación HDMI; el backend
  espera la reconciliación efectiva y no interpreta ese intervalo como fallo.
- El comportamiento final depende del driver WDDM y de la ruta activa; si la
  API devuelve error, el estado se conserva como `unknown`.
