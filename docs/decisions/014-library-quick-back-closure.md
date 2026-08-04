# Decision 014: cierre del incidente Library → Details → Back rápido

## Causa raíz final

El fallo no estaba en el gamepad ni en el teclado virtual. La ruta de Library
tenía dos condiciones que coincidían al cerrar Details rápidamente:

- `ViewTransition` rematerializa la vista primaria y el grid virtualizado
  registra de nuevo sus tarjetas.
- La restauración del opener se solicitaba mientras el scope padre todavía
  atravesaba ciclos de montaje/desmontaje y activación de ruta. El `focusId`
  podía conservarse correctamente, pero el registro y el contexto de la
  tarjeta aún no eran estables para el primer input del gamepad.

La búsqueda filtrada exponía además una segunda causa real: query, status y
sort vivían en `LibraryView` y se reinicializaban al remount. Eso cambiaba el
dataset y podía hacer desaparecer el opener. Era un defecto independiente de
la reproducción mínima sin filtros.

## Corrección aplicada

La corrección aceptada separó ambas responsabilidades:

1. `LibraryStore` es el propietario persistente de sesión de Library para
   query confirmada, status y sort. El dataset continúa derivándose del
   catálogo y esos criterios; no se almacenan nodos, focusables ni contexto de
   navegación.
2. La restauración de Details conserva el contexto del opener en el Engine,
   usa una transacción idempotente identificada por la transición de ruta y
   espera a que el target válido esté registrado antes de comprometer el foco.
3. `ProductShell` conserva el foco por pantalla primaria y coordina la
   activación de ruta con la rematerialización de la vista. El primer input
   posterior converge por el mismo flujo `NAV_INPUT` → `NAV_RESOLVE`.

No se añadieron timeouts, retries, watchdogs ni fallbacks nuevos. El watchdog
y el fallback existentes permanecen como mecanismos generales de recuperación
y observabilidad; no son la solución principal del incidente.

## Regresión protegida

La suite conserva cobertura para:

- Library sin filtros y Library filtrada;
- Back normal y Back rápido desde Details;
- opener exacto y `activeFocusId` registrado, visible y habilitado;
- primer movimiento direccional después de Back y resolución `NAV_RESOLVE`;
- continuidad de query, status/filter y sort, incluida la acción explícita de
  limpiar filtros;
- VirtualKeyboard: commit confirmado y cancelación sin mutar el estado
  confirmado;
- ciclos repetidos, `React.StrictMode`, materialización virtual y un único
  commit de restore.

## Instrumentación de desarrollo

Se conserva `navigationRuntimeTrace` como buffer circular estructurado y el
overlay de desarrollo con `Copy navigation trace`, `Export JSON` y `Clear
trace`. Registra lifecycle, scopes, focus, materialización, restauración,
transiciones, teclado virtual, capacidades del runtime e invariantes, sin
guardar el texto de la búsqueda.

Durante el cierre se retiraron únicamente los emisores `console` redundantes
de la traza de navegación primaria y de `NavigationTrace`. Los eventos
estructurados y los avisos de invariantes siguen disponibles para futuras
regresiones.

## Estado

```text
LIBRARY RESTORE INCIDENT: CLOSED
FUNCTIONAL CHANGES DURING CLOSURE: NONE
```
