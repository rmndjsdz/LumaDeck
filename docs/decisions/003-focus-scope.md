# ADR 003: FocusScope como frontera de navegación

## Decisión

Cada scope declara foco inicial, padre, restauración, scroll recordado y trap
opcional. El engine solo considera candidatos del scope activo.

## Motivo

Los modales necesitan pausar la pantalla inferior y devolver el foco exacto al
cerrarse sin listeners especiales en cada feature.
