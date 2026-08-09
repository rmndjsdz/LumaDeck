import { useCallback, useEffect, useMemo, useState } from "react";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { displayService } from "./display-service";
import {
  canChangeHdr,
  displayErrorMessage,
  hdrStatusLabel,
  modesForResolution,
  sameDisplayMode,
  selectCompatibleMode,
  uniqueModes,
  type DisplayInfo,
  type DisplayMode,
  type DisplayModeChange,
} from "./display-types";

type Picker = "display" | "resolution" | "refresh" | "scale" | "hdr" | null;

export function DisplayView() {
  const { engine } = useNavigation();
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [selectedDisplayId, setSelectedDisplayId] = useState<string | null>(
    null,
  );
  const [modes, setModes] = useState<DisplayMode[]>([]);
  const [picker, setPicker] = useState<Picker>(null);
  const [confirmation, setConfirmation] = useState<DisplayModeChange | null>(
    null,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [remainingSeconds, setRemainingSeconds] = useState(15);
  const [openerFocusId, setOpenerFocusId] = useState<string | null>(null);
  const [pendingHdrDisplayId, setPendingHdrDisplayId] = useState<string | null>(
    null,
  );

  const selectedDisplay = useMemo(
    () => displays.find((display) => display.id === selectedDisplayId) ?? null,
    [displays, selectedDisplayId],
  );
  const currentMode = selectedDisplay?.currentMode ?? null;
  const resolutions = useMemo(() => {
    const seen = new Set<string>();
    return modes.filter((mode) => {
      const key = `${mode.width}x${mode.height}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [modes]);
  const refreshModes = useMemo(
    () =>
      currentMode
        ? modesForResolution(modes, currentMode.width, currentMode.height)
        : [],
    [currentMode, modes],
  );

  const restoreFocus = useCallback(() => {
    const focusId = openerFocusId;
    setOpenerFocusId(null);
    if (!focusId) return;
    window.requestAnimationFrame(() => engine.focus(focusId));
  }, [engine, openerFocusId]);

  const load = useCallback(
    async (keepSelection = true) => {
      try {
        const nextDisplays = await displayService.getDisplays();
        setDisplays(nextDisplays);
        const nextSelected = keepSelection
          ? nextDisplays.find((display) => display.id === selectedDisplayId)?.id
          : undefined;
        setSelectedDisplayId(
          nextSelected ??
            nextDisplays.find((display) => display.primary)?.id ??
            nextDisplays[0]?.id ??
            null,
        );
        setErrorMessage(null);
      } catch (error) {
        setErrorMessage(displayErrorMessage(error));
      }
    },
    [selectedDisplayId],
  );

  useEffect(() => {
    void load(false);
    const timer = window.setInterval(() => void load(), 5000);
    return () => window.clearInterval(timer);
  }, [load]);

  useEffect(() => {
    if (!selectedDisplayId) return;
    let disposed = false;
    void displayService
      .getSupportedDisplayModes(selectedDisplayId)
      .then((nextModes) => {
        if (!disposed) setModes(uniqueModes(nextModes));
      })
      .catch((error) => {
        if (!disposed) setErrorMessage(displayErrorMessage(error));
      });
    return () => {
      disposed = true;
    };
  }, [selectedDisplayId]);

  useEffect(() => {
    if (!confirmation) return;
    const update = () => {
      setRemainingSeconds(
        Math.max(0, Math.ceil((confirmation.expiresAtMs - Date.now()) / 1000)),
      );
    };
    update();
    const timer = window.setInterval(update, 250);
    return () => window.clearInterval(timer);
  }, [confirmation]);

  const openPicker = (nextPicker: Exclude<Picker, null>, focusId: string) => {
    setErrorMessage(null);
    setOpenerFocusId(focusId);
    engine.prepareScopeOpen(`display-${nextPicker}-dialog`, focusId);
    setPicker(nextPicker);
  };

  const closePicker = useCallback(() => {
    setPicker(null);
    restoreFocus();
    return true;
  }, [restoreFocus]);

  const applyMode = useCallback(
    async (mode: DisplayMode, focusId: string) => {
      if (sameDisplayMode(mode, currentMode)) {
        closePicker();
        return;
      }
      setErrorMessage(null);
      try {
        const change = await displayService.beginDisplayModeChange(mode);
        setPicker(null);
        setRemainingSeconds(15);
        setOpenerFocusId(focusId);
        setConfirmation(change);
        engine.prepareScopeOpen("display-confirmation-dialog", focusId);
      } catch (error) {
        setErrorMessage(displayErrorMessage(error));
      }
    },
    [closePicker, currentMode, engine],
  );

  const applyHdr = useCallback(
    async (enabled: boolean) => {
      if (!selectedDisplayId || pendingHdrDisplayId === selectedDisplayId)
        return;
      setErrorMessage(null);
      setPendingHdrDisplayId(selectedDisplayId);
      try {
        await displayService.setHdrEnabled(selectedDisplayId, enabled);
        await load();
        closePicker();
      } catch (error) {
        setErrorMessage(displayErrorMessage(error));
      } finally {
        setPendingHdrDisplayId(null);
      }
    },
    [closePicker, load, pendingHdrDisplayId, selectedDisplayId],
  );

  const closeConfirmation = useCallback(
    async (keep: boolean) => {
      try {
        if (keep) await displayService.confirmDisplayModeChange();
        else await displayService.rollbackDisplayModeChange();
        setConfirmation(null);
        await load();
        restoreFocus();
      } catch (error) {
        setErrorMessage(displayErrorMessage(error));
      }
    },
    [load, restoreFocus],
  );

  useEffect(() => {
    if (!confirmation || remainingSeconds > 0) return;
    const timer = window.setTimeout(() => void closeConfirmation(false), 700);
    return () => window.clearTimeout(timer);
  }, [closeConfirmation, confirmation, remainingSeconds]);

  useEffect(() => {
    if (
      !confirmation ||
      !selectedDisplayId ||
      displays.some((display) => display.id === selectedDisplayId)
    ) {
      return;
    }
    void closeConfirmation(false);
  }, [closeConfirmation, confirmation, displays, selectedDisplayId]);

  const selectedScale = selectedDisplay?.scale;
  const selectedHdr = selectedDisplay
    ? {
        displayId: selectedDisplay.id,
        supported: selectedDisplay.hdrSupported,
        enabled: selectedDisplay.hdrEnabled,
        status: selectedDisplay.hdrStatus,
      }
    : null;
  const hdrPending = pendingHdrDisplayId === selectedDisplayId;
  const title =
    selectedDisplay?.friendlyName ?? selectedDisplay?.name ?? "Pantalla";

  return (
    <section className="display-view" aria-label="Pantalla">
      <header className="settings-heading display-heading">
        <div>
          <p className="eyebrow">Configuración</p>
          <h1>Pantalla</h1>
          <p>Modos reales expuestos por Windows para esta sesión.</p>
        </div>
        <span className="display-platform-note">
          Windows es la fuente de verdad
        </span>
      </header>

      {errorMessage && (
        <p className="settings-feedback is-error" role="alert">
          {errorMessage}
        </p>
      )}

      {displays.length > 1 && (
        <Focusable
          focusId="display-selector"
          scopeId="settings-shell"
          className="display-monitor-card"
          onConfirm={() => openPicker("display", "display-selector")}
          ariaLabel={`Pantalla seleccionada: ${title}`}
        >
          <span className="display-monitor-icon" aria-hidden="true">
            ▣
          </span>
          <span className="display-monitor-copy">
            <small>Pantalla activa</small>
            <strong>{title}</strong>
            <span>{selectedDisplay?.primary ? "Principal" : "Secundaria"}</span>
          </span>
          <span className="display-chevron">›</span>
        </Focusable>
      )}

      <div className="display-summary" aria-live="polite">
        <div>
          <span className="eyebrow">Pantalla</span>
          <strong>{title}</strong>
          <small>{selectedDisplay?.primary ? "Principal" : "Conectada"}</small>
        </div>
        <div className="display-summary-mode">
          <strong>{currentMode ? formatResolution(currentMode) : "—"}</strong>
          <span>
            {currentMode
              ? `${currentMode.refreshRate} Hz`
              : "Modo no disponible"}
          </span>
        </div>
      </div>

      <NavigationGrid
        groupId="display-settings"
        columns={1}
        itemCount={4}
        regionId="settings-content"
        entryFocusId="display-resolution"
        exitFocusId={
          displays.length > 1 ? "display-selector" : "main-nav-settings"
        }
        className="display-settings-list"
      >
        <Focusable
          focusId="display-resolution"
          scopeId="settings-shell"
          gridIndex={0}
          className="display-setting-row"
          onConfirm={() => openPicker("resolution", "display-resolution")}
        >
          <span>
            <small>Resolución</small>
            <strong>{currentMode ? formatResolution(currentMode) : "—"}</strong>
          </span>
          <span className="display-row-meta">{modes.length} modos ›</span>
        </Focusable>
        <Focusable
          focusId="display-refresh"
          scopeId="settings-shell"
          gridIndex={1}
          className="display-setting-row"
          onConfirm={() => openPicker("refresh", "display-refresh")}
        >
          <span>
            <small>Frecuencia</small>
            <strong>
              {currentMode ? `${currentMode.refreshRate} Hz` : "—"}
            </strong>
          </span>
          <span className="display-row-meta">
            {refreshModes.length} válidas ›
          </span>
        </Focusable>
        <Focusable
          focusId="display-scale"
          scopeId="settings-shell"
          gridIndex={2}
          className="display-setting-row"
          onConfirm={() => {
            if (
              selectedScale?.canChange &&
              selectedScale.supported.length > 0
            ) {
              openPicker("scale", "display-scale");
            } else {
              setErrorMessage(
                displayErrorMessage(
                  new Error("DISPLAY_SCALE_CHANGE_UNSUPPORTED"),
                ),
              );
            }
          }}
        >
          <span>
            <small>Escala de Windows</small>
            <strong>
              {selectedScale?.current
                ? `${selectedScale.current} %`
                : "No disponible"}
            </strong>
          </span>
          <span className="display-row-meta">
            {selectedScale?.canChange ? "Cambiar ›" : "Solo consulta"}
          </span>
        </Focusable>
        <Focusable
          focusId="display-hdr"
          scopeId="settings-shell"
          gridIndex={3}
          className="display-setting-row"
          disabled={!canChangeHdr(selectedHdr) || hdrPending}
          onConfirm={() => {
            if (canChangeHdr(selectedHdr)) {
              openPicker("hdr", "display-hdr");
            } else {
              setErrorMessage(
                displayErrorMessage(new Error("DISPLAY_HDR_UNSUPPORTED")),
              );
            }
          }}
        >
          <span>
            <small>HDR</small>
            <strong>{hdrStatusLabel(selectedHdr)}</strong>
          </span>
          <span className="display-row-meta">
            {hdrPending
              ? selectedHdr?.enabled
                ? "Desactivandoâ€¦"
                : "Activandoâ€¦"
              : canChangeHdr(selectedHdr)
                ? "Cambiar â€º"
                : "No disponible"}
          </span>
        </Focusable>
      </NavigationGrid>

      {picker && (
        <NavigationDialog
          scopeId={`display-${picker}-dialog`}
          initialFocusId={
            picker === "display"
              ? "display-option-0"
              : `display-${picker}-option-0`
          }
          className="display-picker-dialog"
          onBack={closePicker}
        >
          <p className="eyebrow">Pantalla · {pickerLabel(picker)}</p>
          <h2>{pickerLabel(picker)}</h2>
          {picker === "display" && (
            <NavigationGrid
              groupId="display-picker"
              columns={1}
              itemCount={displays.length}
              className="display-option-list"
            >
              {displays.map((display, index) => (
                <Focusable
                  key={display.id}
                  focusId={`display-option-${index}`}
                  scopeId={`display-${picker}-dialog`}
                  gridIndex={index}
                  className="display-option"
                  onConfirm={() => {
                    setSelectedDisplayId(display.id);
                    closePicker();
                  }}
                >
                  <span>
                    <strong>{display.friendlyName ?? display.name}</strong>
                    <small>
                      {display.primary ? "Principal" : "Secundaria"} ·{" "}
                      {display.currentMode
                        ? formatResolution(display.currentMode)
                        : "Sin modo"}
                    </small>
                  </span>
                  <span>
                    {display.id === selectedDisplayId ? "Actual" : ""}
                  </span>
                </Focusable>
              ))}
            </NavigationGrid>
          )}
          {picker === "resolution" && (
            <NavigationGrid
              groupId="display-picker"
              columns={1}
              itemCount={resolutions.length}
              className="display-option-list"
            >
              {resolutions.map((mode, index) => (
                <Focusable
                  key={`${mode.width}x${mode.height}`}
                  focusId={`display-resolution-option-${index}`}
                  scopeId={`display-${picker}-dialog`}
                  gridIndex={index}
                  className="display-option"
                  onConfirm={() => {
                    const compatible = selectCompatibleMode(
                      modes,
                      mode.width,
                      mode.height,
                      currentMode?.refreshRate ?? 60,
                    );
                    if (compatible)
                      void applyMode(compatible, "display-resolution");
                  }}
                >
                  <strong>{formatResolution(mode)}</strong>
                  <span>
                    {sameResolution(mode, currentMode) ? "Actual" : ""}
                  </span>
                </Focusable>
              ))}
            </NavigationGrid>
          )}
          {picker === "refresh" && (
            <NavigationGrid
              groupId="display-picker"
              columns={1}
              itemCount={refreshModes.length}
              className="display-option-list"
            >
              {refreshModes.map((mode, index) => (
                <Focusable
                  key={`${mode.width}x${mode.height}-${mode.refreshRate}`}
                  focusId={`display-refresh-option-${index}`}
                  scopeId={`display-${picker}-dialog`}
                  gridIndex={index}
                  className="display-option"
                  onConfirm={() => void applyMode(mode, "display-refresh")}
                >
                  <strong>{mode.refreshRate} Hz</strong>
                  <span>
                    {sameDisplayMode(mode, currentMode) ? "Actual" : ""}
                  </span>
                </Focusable>
              ))}
            </NavigationGrid>
          )}
          {picker === "scale" &&
            (selectedScale?.canChange && selectedScale.supported.length > 0 ? (
              <NavigationGrid
                groupId="display-picker"
                columns={1}
                itemCount={selectedScale.supported.length}
                className="display-option-list"
              >
                {selectedScale.supported.map((scale, index) => (
                  <Focusable
                    key={scale}
                    focusId={`display-scale-option-${index}`}
                    scopeId={`display-${picker}-dialog`}
                    gridIndex={index}
                    className="display-option"
                    onConfirm={() => {
                      if (!selectedDisplayId) return;
                      void displayService
                        .setDisplayScale(selectedDisplayId, scale)
                        .then(() => load())
                        .then(closePicker)
                        .catch((error) =>
                          setErrorMessage(displayErrorMessage(error)),
                        );
                    }}
                  >
                    <strong>{scale} %</strong>
                    <span>
                      {scale === selectedScale.current ? "Actual" : ""}
                    </span>
                  </Focusable>
                ))}
              </NavigationGrid>
            ) : (
              <p className="display-unavailable-note">
                Windows no expone las opciones de escala para esta pantalla en
                esta sesión.
              </p>
            ))}
          {picker === "hdr" && (
            <>
              <NavigationGrid
                groupId="display-picker"
                columns={1}
                itemCount={2}
                className="display-option-list"
              >
                {[false, true].map((enabled, index) => (
                  <Focusable
                    key={String(enabled)}
                    focusId={`display-hdr-option-${index}`}
                    scopeId={`display-${picker}-dialog`}
                    gridIndex={index}
                    className="display-option"
                    disabled={hdrPending}
                    onConfirm={() => void applyHdr(enabled)}
                  >
                    <strong>{enabled ? "Activado" : "Desactivado"}</strong>
                    <span>
                      {selectedHdr?.enabled === enabled ? "Actual" : ""}
                    </span>
                  </Focusable>
                ))}
              </NavigationGrid>
              {hdrPending && (
                <p className="display-unavailable-note" aria-live="polite">
                  {selectedHdr?.enabled
                    ? "Desactivando HDRâ€¦"
                    : "Activando HDRâ€¦"}
                </p>
              )}
            </>
          )}
        </NavigationDialog>
      )}

      {confirmation && (
        <div className="display-confirmation-backdrop">
          <NavigationDialog
            scopeId="display-confirmation-dialog"
            initialFocusId="display-confirm-keep"
            className="display-confirmation-dialog"
            onBack={() => void closeConfirmation(false)}
          >
            <p className="eyebrow">Cambio temporal</p>
            <h2>¿Conservar esta configuración?</h2>
            <p>Windows volverá al modo anterior si no confirmas el cambio.</p>
            <div className="display-confirmation-mode">
              <strong>{formatResolution(confirmation.appliedMode)}</strong>
              <span>{confirmation.appliedMode.refreshRate} Hz</span>
            </div>
            <strong className="display-countdown">{remainingSeconds}</strong>
            <div className="display-confirmation-actions">
              <Focusable
                focusId="display-confirm-keep"
                scopeId="display-confirmation-dialog"
                className="settings-button primary"
                onConfirm={() => void closeConfirmation(true)}
              >
                Conservar
              </Focusable>
              <Focusable
                focusId="display-confirm-revert"
                scopeId="display-confirmation-dialog"
                className="settings-button"
                onConfirm={() => void closeConfirmation(false)}
              >
                Revertir
              </Focusable>
            </div>
            <small className="display-confirmation-hint">
              A: conservar · B: revertir · Enter / Escape
            </small>
          </NavigationDialog>
        </div>
      )}
    </section>
  );
}

function formatResolution(mode: DisplayMode): string {
  return `${mode.width} × ${mode.height}`;
}

function sameResolution(left: DisplayMode, right: DisplayMode | null): boolean {
  return Boolean(
    right && left.width === right.width && left.height === right.height,
  );
}

function pickerLabel(picker: Exclude<Picker, null>): string {
  if (picker === "display") return "Seleccionar pantalla";
  if (picker === "resolution") return "Resolución";
  if (picker === "refresh") return "Frecuencia";
  if (picker === "hdr") return "HDR";
  return "Escala";
}
