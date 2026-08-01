# ADR 007: uso limitado de Zustand

## Decisión

Zustand contiene solo estado observable de navegación e input mode. Registry,
timers, callbacks, rectángulos y snapshots de Gamepad API permanecen
imperativos.

## Motivo

El store es útil para debug y render selectivo, pero no debe convertirse en un
contenedor monolítico de la interacción completa.
