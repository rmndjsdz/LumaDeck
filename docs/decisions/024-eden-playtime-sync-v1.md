# Eden Playtime Sync V1

## Estado

EDEN PLAYTIME SYNC V1: implementado.

## Formato observado

La instalación local configurada resolvió su `dataPath` a:

```text
C:\Users\rmndj\AppData\Roaming\eden
```

LumaDeck no hardcodea esa ruta; parte del `dataPath` de la instalación Eden y
lee únicamente `play_time/playtime.bin`.

La muestra real inspeccionada tenía 64 bytes, cuatro registros de 16 bytes y
remainder cero. Cada registro es:

```text
offset 0..8   : Title ID como u64 little-endian
offset 8..16  : tiempo total como u64 little-endian, en segundos
```

La muestra produjo, entre otros, `0100000000010000 = 21113 segundos =
05:51:53`. Los registros cuyo Title ID no cumple el formato Switch esperado se
ignoran; un registro truncado hace que el playtime se marque como no disponible,
pero no interrumpe la importación de juegos.

## Persistencia y fuente canónica

Se añadió `external_playtime_snapshots`, con:

- `provider = eden`;
- `emulator_installation_id` derivado del ejecutable y `dataPath`;
- `title_id`;
- `game_id` cuando existe correspondencia;
- `total_seconds`, `observed_at` y formato del parser.

Para Eden, el total visible prefiere el snapshot externo más reciente. Las
sesiones de LumaDeck siguen registrándose para Activity, timeline, Last Played
y diagnóstico, pero no se suman al total Eden cuando existe snapshot. Si el
snapshot no está disponible, se conserva el fallback local anterior.

Un valor nuevo menor que el snapshot persistido no reduce silenciosamente el
total: se registra como anomalía y se conserva el valor mayor.

## Reconciliación

Rescan Eden ejecuta: roots → juegos → Title IDs → `playtime.bin` → snapshots.
El cierre confirmado de una sesión iniciada desde LumaDeck ejecuta la misma
sincronización como punto de actualización automático. Una lectura repetida del
mismo valor no acumula tiempo. Los valores aumentados reemplazan el snapshot
anterior. El archivo es read-only para LumaDeck.

`launched.json` continúa aportando Last Played histórico durante Discovery;
`eden_log` continúa apoyando la resolución de Title IDs cuando el archivo del
juego no contiene uno. Ninguno de los dos se usa para sumar playtime.

## LUDEX REUSE REPORT

| Elemento                       | Clasificación | Motivo                                                                                                                          |
| ------------------------------ | ------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `play_time/playtime.bin`       | `ADAPTED`     | Se reutiliza la idea y estructura previa, pero con parser aislado, validación de longitud, little-endian explícito y snapshots. |
| `launched.json`                | `REUSED`      | Solo para Last Played histórico.                                                                                                |
| `eden_log`                     | `REUSED`      | Solo para asociación histórica de Title ID.                                                                                     |
| suma de sesiones al total Eden | `REJECTED`    | Produce double-count cuando Eden ya incorpora la sesión.                                                                        |

## Limitaciones

V1 soporta una instalación Eden configurada. El sync automático de startup no
se añade para no bloquear el arranque; el cierre de sesión y Rescan son los
puntos de reconciliación. La validación con Mario Odyssey y una sesión real de
Eden queda pendiente.
