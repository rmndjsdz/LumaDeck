# Display Profile V1

## Decisión

Display Profile se integra en el menú contextual existente de Game Details y
persiste un único perfil por `game_id` en SQLite. La configuración `Auto` no
realiza llamadas nativas ni cambia el escritorio.

En Windows V1 usa la pantalla principal (`DISPLAY1` cuando Windows no marca
otra como primaria), enumera modos con `EnumDisplaySettingsExW` y solo aplica
un `DEVMODEW` previamente enumerado después de `CDS_TEST`. El cambio se hace
sin `CDS_UPDATEREGISTRY`.

## Restauración

Antes de aplicar un modo se guarda `pending_display_restore`. El tracking
existente de `SteamGameSessionService` decide cuándo termina el proceso real,
incluyendo su ventana de desaparición/reemplazo, y en ese momento restaura el
modo si `restore_on_exit` está activo. Los fallos de lanzamiento restauran de
inmediato. Al iniciar LumaDeck se intenta resolver cualquier restauración
pendiente antes de permitir otro lanzamiento.

La limitación actual del tracking se conserva: juegos con launchers
intermedios o anti-cheat detectados por las reglas existentes se marcan como
no compatibles y no se lanzan mediante este flujo.
