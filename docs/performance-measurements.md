# Mediciones de rendimiento

## Instrumentación

En desarrollo se marcan input recibido, foco lógico, foco DOM confirmado,
scroll completado, contenido principal, solicitud/decodificación/crossfade de
fondo y vista activa. El HUD muestra una ventana de 500 ms y cuenta frames
fuera de 16.67 ms, long tasks, tarjetas montadas y renders acumulados.

La instrumentación no publica datos analógicos del gamepad ni mantiene un loop
adicional por componente. PerformanceObserver y requestAnimationFrame tienen
cleanup y el HUD se omite en producción.

## Resultados de esta pasada

| Medición               |                                Base |                                        Después |
| ---------------------- | ----------------------------------: | ---------------------------------------------: |
| pruebas Vitest         |                                  39 |                                             45 |
| tarjetas Home montadas |                                  15 |                                             15 |
| frame medio observado  |                             16.7 ms |                pendiente de captura comparable |
| peor frame observado   |                             17.0 ms |                pendiente de captura comparable |
| HUD por defecto        |                      visible en dev |                                    desactivado |
| actualización de foco  | todos los focusables recibían el id | selector booleano: solo anterior/nuevo cambian |

La comparación de FPS debe repetirse a 2560×1440 con el mismo navegador y
catálogo. Esta implementación aporta las marcas y el HUD para capturar esa
medición sin convertirla en una afirmación sintética.
