# Global Emulator Session Control V1

## Estado

`GLOBAL EMULATOR SESSION CONTROL V1: IMPLEMENTED`

`START + SELECT HOLD 2S: IMPLEMENTED`

`EDEN: INTEGRATED`

`CEMU: READY FOR ADAPTER`

`DOLPHIN: READY FOR ADAPTER`

`RPCS3: READY FOR ADAPTER`

`REAL APPLICATION QA: PENDING`

## Contrato

Mientras existe una sesión de emulador en estado `Running`, mantener `Start +
Select` durante 2 segundos expresa una intención común de `StopGameSession`.
El monitor no conoce Eden ni termina procesos directamente. La capa de
`GameSession` resuelve el cierre del emulador activo.

La detección corre en Rust mediante XInput, no depende del foco de la ventana
de LumaDeck ni de eventos React. El monitor solo vive durante la sesión emulada
y se detiene al pasar a `Finishing` o `Ready`.

## Máquina del shortcut

```text
IDLE -> HOLDING -> WAIT_RELEASE -> IDLE
```

- ambos botones deben pertenecer al mismo controller;
- el umbral es `2_000 ms` en un único token;
- soltar antes del umbral cancela;
- una pulsación larga dispara una sola vez;
- desconectar cancela el hold;
- reconectar exige liberar los botones antes de iniciar otro hold;
- no hay combinación entre dos controllers.

La implementación consulta los cuatro índices XInput y registra únicamente
transiciones significativas; no registra cada lectura.

## Cierre de sesión

Eden utiliza el conjunto de PID asociado a la sesión, incluyendo procesos
nuevos de handoff y un proceso preexistente solo cuando su command line está
asociada al juego actual.

1. Se envía `WM_CLOSE` a las ventanas de esos PID.
2. Se espera hasta 4 segundos.
3. Si permanecen vivos, se usa `TerminateProcess` únicamente sobre esos PID.
4. Se deja que Activity finalice una sola vez.
5. Eden `play_time.bin` se sincroniza y LumaDeck restaura `READY`.

No se usa `taskkill`, `cmd.exe`, PowerShell para terminar procesos, ni cierre
por nombre de imagen.

## Diagnóstico local

Se reutiliza:

```text
%APPDATA%\com.lumadeck.desktop\settings-runtime.log
```

La infraestructura rota el archivo al superar 5 MiB y conserva una copia como
`settings-runtime.log.1`.

Para reconstruir una sesión, buscar:

```text
sessionId=play-...
```

Eventos relevantes:

- `SESSION_CREATED`, `SESSION_STATE_CHANGED`;
- `GAMEPAD_MONITOR_STARTED`, `GAMEPAD_CONTROLLER_CONNECTED`;
- `GAMEPAD_SHORTCUT_HOLD_STARTED`, `GAMEPAD_SHORTCUT_HOLD_CANCELLED`;
- `GAMEPAD_SHORTCUT_THRESHOLD_REACHED`, `GAMEPAD_SHORTCUT_TRIGGERED`;
- `SESSION_STOP_REQUESTED`;
- `EMULATOR_GRACEFUL_CLOSE_REQUESTED`, `EMULATOR_GRACEFUL_CLOSE_SUCCEEDED`;
- `EMULATOR_GRACEFUL_CLOSE_TIMEOUT`;
- `EMULATOR_FORCE_TERMINATE_REQUESTED`,
  `EMULATOR_FORCE_TERMINATE_SUCCEEDED`;
- `PROCESS_PID_ATTACHED`, `PROCESS_PID_DETACHED`;
- `SESSION_FINISHED`, `SESSION_STOP_COMPLETED`, `READY_RESTORED`;
- `EDEN_PLAYTIME_SYNC_REQUESTED`, `EDEN_PLAYTIME_SYNC_COMPLETED` y
  `EDEN_PLAYTIME_SYNC_ERROR`.

Los eventos nuevos incluyen `sessionId`, `gameId`, `emulator`, controller o
PID cuando corresponde. No se escriben rutas de ROM en el flujo del shortcut.

## Overlay

No se añade una ventana topmost nueva para mostrar el progreso sobre un
emulador fullscreen. Esto evita hacks de foco o comportamiento invasivo. La
salida funciona sin overlay; el feedback queda en el log y en el estado de
sesión de LumaDeck. Un overlay seguro puede añadirse posteriormente si la
infraestructura de ventanas existente lo permite.

## AV-FRIENDLY IMPLEMENTATION REPORT

- sin DLL injection;
- sin global keyboard hooks;
- sin driver;
- sin proceso auxiliar residente;
- sin privilegios de administrador;
- sin Defender exclusions;
- XInput estándar para gamepad;
- `WM_CLOSE`/`EnumWindows` para cierre normal;
- `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess` para fallback;
- terminación limitada a PID asociados a la sesión;
- sin `taskkill`, `cmd.exe` ni WMI para terminar procesos;
- sin manipulación de memoria.

El monitor existente de detección de handoff de Eden conserva su consulta
oculta de procesos de la implementación previa; el nuevo stop flow no añade
PowerShell ni lo usa para terminar procesos.

## QA real

Con Eden fullscreen:

1. lanzar Super Mario Odyssey o Mario Kart desde LumaDeck;
2. mantener Start + Select durante 1 segundo y soltar: no debe cerrar;
3. repetir durante más de 2 segundos: debe registrar
   `GAMEPAD_SHORTCUT_TRIGGERED`;
4. comprobar cierre normal, sincronización de playtime y `READY_RESTORED`;
5. repetir con Eden ya abierto;
6. desconectar y reconectar el mando durante el hold;
7. mantener la combinación 5 segundos: debe existir un único trigger;
8. probar un segundo controller y verificar que no complete el hold del
   primero.

El log a entregar para soporte es `settings-runtime.log`; indicar el valor de
`sessionId` y pedir el análisis de esa sesión.
