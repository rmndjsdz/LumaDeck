# ADR 002: Focus Registry imperativo

## Decisión

El registry guarda nodos, metadatos y rectángulos en una estructura imperativa;
Zustand solo refleja el foco seleccionado y métricas pequeñas.

## Motivo

Registrar, medir o mover elementos no debe provocar renders globales ni guardar
árboles React, callbacks de alta frecuencia o eventos nativos.
