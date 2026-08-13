# NVIDIA OPS Provider V1

LumaDeck integra NVIDIA Optimal Playable Settings (OPS) como una fuente
local, opcional y read-only para el perfil gráfico sugerido.

## Alcance

El proveedor lee únicamente los archivos que NVIDIA App ya mantiene en
`%LOCALAPPDATA%\NVIDIA Corporation\NVIDIA App\NvBackend\`:

- `ApplicationStorage.json` para identificar el juego.
- `Recommendations\<shortName>\...\pops.pub.tsv` para los perfiles.
- `metadata.json` asociado para seleccionar el POP por resolución.

El archivo `pops.pub.tsv` se valida como JSON sin confiar en su extensión.
La selección `pops` se trata como one-indexed, tal como aparece en la
metadata de NVIDIA: `pops: 1` selecciona el primer elemento del arreglo.

`ApplicationState`, `Position`, `Original.Values`, los wrappers Lua y los
archivos de configuración del juego no son fuentes obligatorias. Current
Settings queda fuera de esta V1; la recomendación sigue disponible aunque el
parser específico del juego no pueda leer sus valores actuales.

## Resolución y normalización

El matching usa esta prioridad:

1. Steam AppID extraído de `LaunchCmd`.
2. Ruta del ejecutable.
3. Título exacto como fallback conservador.

Cuando la resolución física del display apunta a un perfil marcado
`belowMinSpec`, el proveedor busca una resolución no marcada con la misma
relación de aspecto y dentro del ancho disponible. Si no existe, conserva el
perfil explícito de NVIDIA y su marca `belowMinSpec`.

Los settings se devuelven con `canonicalKey`, nombre visible, valor normalizado
y valor crudo. Se separan tecnología y modo de upscaling; Frame Generation de
NVIDIA RTX y FSR Frame Generation se conservan como tecnologías distintas.
Los valores desconocidos no se convierten en inferencias.

## Integración y fallback

“Perfil gráfico sugerido” intenta primero NVIDIA OPS. Si NVIDIA App, el
catálogo, el paquete o el JSON no están disponibles, mantiene el resolver
actual de LumaDeck. Ningún estado de OPS inválido rompe la pestaña Rendimiento.

La UI muestra `Fuente: NVIDIA OPS` cuando la respuesta es válida y muestra un
aviso discreto cuando `belowMinSpec=true`. No muestra todos los settings en
V1, pero el modelo los conserva para futuras expansiones.

## Seguridad y riesgos

LumaDeck does not call NVIDIA private cloud endpoints and does not modify
NVIDIA App or game settings in OPS V1.

La integración depende de un formato local no documentado oficialmente. Por
eso valida JSON, tolera campos desconocidos, devuelve estados explícitos y
mantiene el fallback existente. No implementa Optimize, Apply, restauración,
slider, wrappers Lua, NVAPI DRS, cloud fetch, benchmarking ni telemetría.
