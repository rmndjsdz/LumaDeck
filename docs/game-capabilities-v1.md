# Game Capabilities V1

## Responsabilidades

`PCGamingWikiProvider` responde qué evidencia publica PCGamingWiki. La capa
`GameCapabilityResolver` responde qué conoce LumaDeck después de aplicar
precedencia y overrides. Esta capa no activa HDR, DLSS, Frame Generation,
Lossless Scaling ni ningún ajuste de GPU/pantalla.

```text
PCGamingWiki evidence + user overrides
                 ↓
       GameCapabilityResolver
                 ↓
       ResolvedGameCapabilities
```

La evidencia PCGamingWiki existente se reutiliza; no existe una segunda tabla
de evidence. La única persistencia nueva es `game_capability_overrides`, creada
por la migración forward-only 29.

## Dominio

V1 soporta únicamente:

- `NATIVE_HDR`
- `HIGH_FIDELITY_UPSCALING`
- `FRAME_GENERATION`
- `FOUR_K`
- `SIXTY_FPS`
- `HIGH_REFRESH_120_FPS`

Cada resultado conserva `YES`, `NO` o `UNKNOWN`, confianza, fuente,
tecnologías, `alternativeAvailable`, `sourceNote`, evidencia ganadora,
evidencia restante, `stale` y conflicto.

Las capacidades de video (`FOUR_K`, `SIXTY_FPS` y `HIGH_REFRESH_120_FPS`) no
representan la resolución actual del monitor, la frecuencia de refresco ni FPS
estimados del hardware. Una nota como `Capped to 60 FPS.` se conserva como
`sourceNote` de `HIGH_REFRESH_120_FPS`.

`NO` describe ausencia de soporte nativo/out-of-the-box reportado; no significa
que la capability sea imposible mediante una alternativa. La nota se conserva
sin ejecutar ni interpretar el workaround. Una alternativa nunca cambia el
estado de `NATIVE_HDR`: `NO + alternativeAvailable=YES` sigue siendo HDR nativo
no compatible.

La precedencia es independiente de confidence:

```text
USER_OVERRIDE > PCGAMINGWIKI > NONE
```

Los estados de override son `NO_OVERRIDE`, `FORCE_YES`, `FORCE_NO` y
`FORCE_UNKNOWN`. Limpiar un override elimina únicamente esa fila y vuelve a
resolver la evidencia almacenada; no fuerza una consulta HTTP.

## Servicios y UI

El backend expone `get_game_capabilities`, `refresh_game_capabilities`,
`set_game_capability_override` y `clear_game_capability_override`.
Details usa una única query TanStack Query por juego/identidad. Cambiar o
limpiar un override actualiza esa query localmente, sin invalidar la caché HTTP
de PCGamingWiki.

El selector usa `NavigationDialog` y `FocusScope`: Automático, Sí, No y
Desconocido funcionan con mouse, teclado y gamepad. Al cerrar se restaura el
focus de la capability que abrió el selector.

## QA

Marvel Tōkon conserva la evidencia real: HDR `NO/HIGH`, upscaling `YES/HIGH`
con `TSR`, `DLSS 4`, `NIS`, `FSR 4`, `XeSS 2`, y frame generation `NO/HIGH`.
Para HDR y frame generation, la evidencia conserva `alternativeAvailable=YES`
y la nota de PCGamingWiki sobre engine/glossary/workarounds.
Un override HDR `YES` resuelve `YES/USER_OVERRIDE` y registra conflicto; al
limpiarlo vuelve a `NO/PCGAMINGWIKI` sin request adicional mientras la evidencia
permanezca válida.

Carrion conserva `NO/HIGH`, `UNKNOWN/LOW`, `UNKNOWN/LOW`; `UNKNOWN` se muestra
como `Desconocido`, nunca como `No`.

Eventos principales: `capabilities.resolve.start`,
`capabilities.resolve.result`, `capabilities.conflict`,
`capabilities.override.set`, `capabilities.override.clear`,
`capabilities.refresh.start` y `capabilities.refresh.complete`.
