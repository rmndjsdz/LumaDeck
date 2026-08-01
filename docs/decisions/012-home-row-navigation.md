# Navegación lógica entre filas

Home conserva un único scope navegable (`product-shell`). Sus categorías son
filas de un `NavigationRowGroup`, no `FocusScope` independientes. Cada tarjeta
registra un `focusId` estable, `rowId`, `rowIndex` e `itemIndex` lógico.

## Contrato

`NavigationRow` mantiene la navegación lineal horizontal de su propia fila y
no intercepta `Up`/`Down`. `NavigationRowGroup` declara el grupo vertical y
`NavigationRowCoordinator` conserva imperativamente el último foco por fila y
la posición horizontal preferida; no guarda nodos DOM en Zustand.

La navegación vertical sigue esta prioridad determinista:

1. último foco de la fila destino cuando la acción sigue a una restauración;
2. mismo `preferredItemIndex`;
3. centro horizontal más cercano;
4. último elemento disponible en una fila corta;
5. primer elemento válido como último fallback.

La fila corta solo cambia el índice efectivo del destino. El
`preferredItemIndex` original permanece para poder recuperar la columna al
volver a una fila más larga. La geometría aporta `centerX` como criterio
secundario, nunca como autoridad vertical.

Details suspende el scope padre y, al cerrarse, restaura el `focusId` del
opener. El coordinador reconstruye la fila, el índice y la posición preferida
antes de aceptar el siguiente `Up`/`Down`.

Library mantiene sin cambios su índice absoluto, virtualización y
`gridNavigation`.
