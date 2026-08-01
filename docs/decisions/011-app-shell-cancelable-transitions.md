# ADR 011: App Shell y transiciones cancelables

## Estado

Aceptado.

## Decisión

El App Shell y BackgroundManager permanecen montados. Home, Library y Details
se presentan dentro de `ViewTransition`; cada solicitud incrementa un token de
vista y remonta solo el wrapper visual para cancelar la animación anterior.
La activación de scope/foco sigue siendo responsabilidad del engine.

## Consecuencias

Back y confirm no quedan bloqueados por la salida visual, no se reinicia el
background al cambiar de vista y Details conserva su foco inicial válido.
Los contenidos de la vista anterior se desmontan para evitar focusables
duplicados y trabajo oculto durante la transición.
