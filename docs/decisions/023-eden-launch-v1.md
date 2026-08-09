# Eden Launch V1

## Estado

EDEN LAUNCH V1: implementado en el flujo común de `Play`.

## Decisión

LumaDeck resuelve el destino de lanzamiento desde el registro local del juego.
Steam conserva su estrategia `steam://rungameid/{appId}` y Eden usa el mismo
servicio de sesión con una estrategia específica del proveedor:

```text
eden.exe -f -g <game-path>
```

Los argumentos se pasan como argumentos separados de `Command`; no se usa
shell ni se escribe configuración persistente de Eden. La documentación actual
de Eden confirma `-g <path>` para cargar un juego y `-f` para pantalla completa:
<https://github.com/eden-emulator/mirror/blob/master/docs/user/CommandLine.md>.

## Validación y seguimiento

Antes de lanzar se valida la instalación guardada, el PE `eden.exe`, la
existencia del archivo `.nsp`/`.xci` y que permanezca dentro de una raíz de
biblioteca configurada. El monitor local distingue:

- proceso de Eden ya existente;
- PID nuevo del lanzamiento;
- handoff/delegación detectado por la línea de comandos que contiene el juego;
- juego detectado;
- proceso de emulador vivo sin evidencia de una sesión de juego.

Un handoff no inicia otra sesión. `game_sessions` continúa siendo la fuente de
actividad: su creación es idempotente por juego, el cierre se aplica una sola
vez, `get_local_games` suma su duración y `last_played_at` se actualiza al
cierre. Las sesiones activas que quedaron persistidas al iniciar LumaDeck se
cierran como interrumpidas con duración cero para no inflar playtime.

Los eventos diagnósticos se escriben en el log local existente. La información
sanitizada de Eden contiene plataforma, emulador, game ID, title ID y resultado;
no contiene rutas completas. No se añadió una segunda plataforma de
telemetría remota.

## Reporte de reutilización LUDEX

| Elemento                                     | Clasificación | Uso en Eden V1                                                                                    |
| -------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------------- |
| `inspectEdenGameProcesses`/probe equivalente | `ADAPTAR`     | Consulta PID y línea de comandos, con handoff y separación entre emulador vivo y juego detectado. |
| argumentos de lanzamiento                    | `ADAPTAR`     | Se reutiliza `Command` seguro; Eden recibe `-f -g` y Steam conserva URI.                          |
| handoff                                      | `REUTILIZAR`  | Una única `GameSessionStatus` y un único registro `game_sessions`.                                |
| `playtime.bin`                               | `REUTILIZAR`  | Solo como playtime histórico de Discovery; las sesiones nuevas usan actividad LumaDeck.           |
| `launched.json`                              | `REUTILIZAR`  | Last played histórico durante Discovery; no se usa como prueba de juego activo.                   |
| `eden_log`                                   | `REUTILIZAR`  | Discovery usa el log para title IDs; Launch V1 no lo trata como prueba suficiente de sesión.      |
| session tracking                             | `REUTILIZAR`  | `game_session`/`game_sessions` existentes, con cierre idempotente y recuperación stale.           |

## Limitaciones y QA

V1 soporta una instalación Eden configurada actualmente. Si Eden permanece
abierto y deja de exponer el juego en su línea de comandos, LumaDeck no asume
que el proceso sea una sesión activa; la sesión pasa a finalización. La
validación real con un juego debe confirmar el comportamiento de handoff de la
build instalada, cierre normal, crash y Eden ya abierto. La versión de archivo
local no expone metadatos de producto, por lo que esa comprobación queda para
QA real.
