# Decisión 027 — Configuración de pantalla V1

## Estado

Implementada parcialmente en la pantalla `Configuración → Pantalla`.

## Arquitectura

La UI usa `display-service.ts` y no conoce Win32. Los comandos Tauri delegan
en `src-tauri/src/display.rs`, que enumera los displays activos mediante
`EnumDisplayDevicesW`, consulta modos completos por display con
`EnumDisplaySettingsExW` y aplica cambios con `ChangeDisplaySettingsExW`.

Los modos siempre conservan la tupla completa `displayId + width + height +
refreshRate`. Así, las frecuencias mostradas para una resolución son
exactamente las combinaciones que Windows expone para esa pantalla. El estado
posterior se vuelve a consultar antes de informar éxito.

## Confirmación y rollback

Antes de aplicar un modo se persiste el modo anterior en la tabla de
recuperación existente y se arma un temporizador administrado por Rust de 15
segundos. El backend restaura automáticamente el modo si no llega una
confirmación. La recuperación al iniciar también restaura cualquier entrada
pendiente que haya quedado después de un cierre del proceso.

La UI muestra el countdown y permite `Conservar` o `Revertir`, pero no es la
única responsable de la seguridad. Los cambios se rechazan si el modo ya no
está enumerado o si la verificación posterior no coincide.

## Escala de Windows

La escala efectiva, recomendada y el rango permitido se consultan por monitor
mediante la extensión interna de `DisplayConfigGetDeviceInfo` para DPI. El
cambio se aplica con el paquete equivalente de `DisplayConfigSetDeviceInfo`.
Windows no documenta los tipos `-3` y `-4` usados por estas operaciones, por
lo que el backend valida el tamaño de las estructuras, limita el valor a los
porcentajes que Windows devuelve y devuelve un error si el sistema rechaza la
operación. No se edita el registro ni se reinicia Explorer.

`DisplayScale` expone `current`, `recommended`, `supported` y `canChange`.
Si la extensión no está disponible en una versión concreta de Windows, la UI
queda en modo consulta y muestra el error real del backend.

## Fuera de alcance

HDR, topología de pantallas, monitor principal, perfiles por juego y cambios de
escala mediante hacks o automatización de la aplicación Configuración quedan
para una fase posterior.
