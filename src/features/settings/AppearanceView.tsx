import { useEffect, useState } from "react";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { getAvailableThemeDescriptors } from "../../ui/theme/theme-registry";
import { useTheme } from "../../ui/theme/theme-context";
import type { ThemeDescriptor } from "../../ui/theme/theme-types";
import "./appearance-theme.css";

export function AppearanceView() {
  const {
    confirmedTheme,
    previewThemeId,
    setTheme,
    previewTheme,
    clearThemePreview,
  } = useTheme();
  const themes = getAvailableThemeDescriptors();
  const [focusedThemeId, setFocusedThemeId] = useState(confirmedTheme.id);

  useEffect(() => () => clearThemePreview(), [clearThemePreview]);

  return (
    <>
      <div className="settings-heading">
        <div>
          <p className="eyebrow">Configuración · Apariencia</p>
          <h1 id="settings-heading">Apariencia</h1>
          <p>Personaliza el aspecto de LumaDeck</p>
        </div>
        <span className="page-hint">A seleccionar · B atrás</span>
      </div>
      <section
        className="appearance-settings-layout"
        aria-labelledby="theme-heading"
      >
        <div className="settings-panel">
          <div className="settings-panel-heading">
            <div>
              <p className="eyebrow">Tema</p>
              <h2 id="theme-heading">TEMA</h2>
            </div>
          </div>
          <NavigationGrid
            groupId="appearance-themes"
            columns={1}
            itemCount={themes.length}
            regionId="settings-content"
            entryFocusId={`appearance-theme-${themes[0]?.theme.id ?? confirmedTheme.id}`}
            exitFocusId="settings-appearance"
            className="appearance-theme-list"
          >
            {themes.map((descriptor, index) => {
              const availableTheme = descriptor.theme;
              const active = availableTheme.id === confirmedTheme.id;
              const focused = availableTheme.id === focusedThemeId;
              const previewing = availableTheme.id === previewThemeId;
              return (
                <Focusable
                  key={availableTheme.id}
                  focusId={`appearance-theme-${availableTheme.id}`}
                  scopeId="settings-shell"
                  gridIndex={index}
                  className={`settings-card is-enabled appearance-theme-card${focused ? " is-preview-focused" : ""}${previewing ? " is-previewing" : ""}`}
                  aria-label={`${availableTheme.name}${active ? ", Activo" : ""}`}
                  onFocus={() => {
                    setFocusedThemeId(availableTheme.id);
                    previewTheme(availableTheme.id);
                  }}
                  onConfirm={() => setTheme(availableTheme.id)}
                >
                  <ThemePreview descriptor={descriptor} />
                  <span className="settings-card-copy">
                    <strong>{availableTheme.name}</strong>
                    <small>{availableTheme.description}</small>
                  </span>
                  <span className="settings-card-arrow">
                    {active ? "Activo" : "Seleccionar"}
                  </span>
                </Focusable>
              );
            })}
          </NavigationGrid>
        </div>
      </section>
    </>
  );
}

function ThemePreview({ descriptor }: { descriptor: ThemeDescriptor }) {
  return (
    <div
      className={`theme-preview theme-preview-${descriptor.preview.home}`}
      aria-hidden="true"
    >
      <div className="theme-preview-hero">
        <span className="theme-preview-logo">
          {descriptor.preview.home === "cinematic" ? "CINEMATIC" : "LumaDeck"}
        </span>
        <span className="theme-preview-fade" />
      </div>
      {descriptor.preview.home === "cinematic" ? (
        <>
          <div className="theme-preview-metadata">
            <span>Adventure</span>
            <span>2026</span>
            <span>★ 84</span>
            <span>♜ 11 / 49</span>
            <span>Steam</span>
          </div>
          <div className="theme-preview-rail">
            {Array.from({ length: 8 }, (_, index) => (
              <span key={index} />
            ))}
          </div>
        </>
      ) : (
        <div className="theme-preview-default-rows">
          <div />
          <div />
        </div>
      )}
    </div>
  );
}
