# ADR 005: foco lógico separado de la presentación

## Decisión

El engine mantiene `activeFocusId`; los componentes reflejan ese estado con
atributos `data-*`, `tabIndex` y CSS según `data-input-mode`.

## Motivo

La misma navegación puede tener estilos distintos para mouse, teclado y
gamepad sin que el dominio conozca el dispositivo.
