# Presupuesto de rendimiento

Library mantiene como máximo 60 tarjetas montadas para que el catálogo de 200
no convierta cada movimiento en trabajo proporcional al total. La resolución
espacial habitual debe ser menor de 2 ms, la respuesta visual debe aparecer en
el siguiente frame y no debe existir un render global por cada frame de
Gamepad API.

El registry cachea `DOMRect` y comparte `ResizeObserver`. Zustand recibe solo
acciones discretas, foco, input mode y métricas pequeñas; el estado analógico
del gamepad vive en su adapter imperativo. El overlay de desarrollo no está
incluido en producción ni debe convertirse en una fuente de trabajo de alta
frecuencia.

## Presupuesto perceptivo

Los tokens visuales objetivo son: focus 132 ms, standard 184 ms, panel 212 ms,
entrada de vista 248 ms, salida 168 ms y crossfade de fondo 304 ms. El foco
usa `transform` y bordes; no anima ancho, alto ni posiciones del grid. Durante
`fast-navigating` el scroll usa comportamiento inmediato y el fondo conserva la
capa actual.

El HUD detallado se activa con `?hud=detail` y registra FPS, frame time medio,
peor frame, frames sobre 16.67 ms, long tasks, nodos montados, renders del
shell/tarjetas, fase de navegación y estado de fondo. El HUD está desactivado
por defecto y se elimina en producción.

## Métrica base de auditoría

En la ejecución inicial del 31 de julio de 2026, Home reportó 60 FPS, 16.7 ms
de frame medio, 17.0 ms de peor frame y 15 tarjetas montadas en el viewport de
auditoría. La suite inicial tenía 39 pruebas. Son valores de referencia de la
sesión, no una captura comparable a 2560×1440.
