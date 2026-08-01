# ADR 004: navegación espacial pura

## Decisión

La elección de vecino es una función pura que combina dirección, solapamiento,
distancia principal, distancia perpendicular, prioridad y `focusId`.

## Motivo

La lógica puede probarse sin DOM, es determinista y funciona aunque cambien las
dimensiones visuales de una grid.
