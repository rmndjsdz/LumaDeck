# Invariantes de la cuadrícula virtual de Library

Library conserva una cuadrícula lógica absoluta de cinco columnas. El índice
de una tarjeta es el índice del catálogo filtrado, no su posición dentro de la
ventana montada. Por tanto:

- `row = floor(index / columns)`;
- `column = index % columns`;
- bajar y subir suman o restan `columns`;
- izquierda y derecha cambian una columna solo dentro de la misma fila.

La última fila es estricta: si la misma columna no existe, la acción vertical
no mueve el foco. No se selecciona otra columna como sustituto.

Cuando el destino no está montado, el engine conserva un
`pendingFocusRequest` con el índice absoluto, `focusId`, dirección, columna y
un `requestId`. Library solo cambia la ventana; no el foco. El registry debe
registrar el `focusId` solicitado antes de que el engine lo active en el
siguiente `requestAnimationFrame`. Una solicitud nueva cancela la anterior y
no se ejecuta navegación espacial sobre el subconjunto montado.

La ventana mantiene doce filas y se mueve por filas completas con dos filas de
overscan lógico. No centra automáticamente el destino. Antes de cambiarla,
Library captura el foco ancla y su posición; después compensa `scrollTop` para
conservar esa posición. El `FocusScrollManager` solo aplica el desplazamiento
adicional necesario para que el destino quede visible.

Las keys y los IDs de foco de las tarjetas se basan en `game.id`. La ventana
mantiene como máximo 60 tarjetas montadas para el catálogo actual de 200
elementos.
