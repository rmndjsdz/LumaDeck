import { useEffect, useRef, useState } from "react";

import { useNavigation } from "../../ui/navigation/navigation-context";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { NavigationList } from "../../ui/navigation/layouts/NavigationList";
import { NavigationRow } from "../../ui/navigation/layouts/NavigationRow";
import { NavigationTabs } from "../../ui/navigation/layouts/NavigationTabs";
import { ScrollRestoration } from "../../ui/navigation/scroll/scroll-restoration";
import { useNavigationStore } from "../../stores/navigation-store";
import { NavigationDebugOverlay } from "../../ui/navigation/debug/NavigationDebugOverlay";

type DemoSection = "overview" | "library" | "settings";

const sectionLabels: Record<DemoSection, string> = {
  overview: "Overview",
  library: "Grid Lab",
  settings: "Settings",
};

export function NavigationDemo() {
  const { engine } = useNavigation();
  const [section, setSection] = useState<DemoSection>("overview");
  const [modalOpen, setModalOpen] = useState(false);
  const [confirmedAction, setConfirmedAction] = useState("None yet");
  const previousSection = useRef(section);
  const modalOpener = useRef<string | null>(null);
  const inputMode = useNavigationStore((state) => state.inputMode);

  useEffect(() => {
    if (previousSection.current === section) return;
    previousSection.current = section;
    const initialFocusId =
      section === "overview"
        ? "overview-action-0"
        : section === "library"
          ? "library-item-1"
          : "settings-action-0";
    engine.focus(initialFocusId);
  }, [engine, section]);

  useEffect(() => {
    if (modalOpen || !modalOpener.current) return;
    const focusId = modalOpener.current;
    modalOpener.current = null;
    engine.focus(focusId);
  }, [engine, modalOpen]);

  const openModal = () => {
    modalOpener.current = engine.getActiveFocusId();
    engine.prepareScopeOpen("demo-modal", modalOpener.current ?? undefined);
    setModalOpen(true);
  };

  const confirm = (message: string) => {
    setConfirmedAction(message);
  };

  return (
    <div className="demo-page">
      <header className="demo-header">
        <div>
          <p className="eyebrow">LumaDeck / Navigation Lab</p>
          <h1>La navegación es el producto.</h1>
          <p className="lede">
            Una demostración pequeña donde mouse, teclado y gamepad comparten el
            mismo motor de foco.
          </p>
        </div>
        <div className="status-card" aria-live="polite">
          <span className="status-dot" />
          <span>Input mode</span>
          <strong>{inputMode}</strong>
        </div>
      </header>

      <FocusScope
        scopeId="demo-app"
        initialFocusId="tab-overview"
        restoreFocus
        rememberScroll
        activateOnMount
      >
        <NavigationTabs className="demo-tabs">
          {(Object.keys(sectionLabels) as DemoSection[]).map((item) => (
            <Focusable
              key={item}
              focusId={`tab-${item}`}
              scopeId="demo-app"
              className="demo-tab"
              onConfirm={() => setSection(item)}
            >
              <span>{sectionLabels[item]}</span>
            </Focusable>
          ))}
        </NavigationTabs>

        <main className="demo-content">
          <div className="demo-content-heading">
            <div>
              <p className="eyebrow">Interactive proof</p>
              <h2>{sectionLabels[section]}</h2>
            </div>
            <span className="scope-pill">scope: demo-app</span>
          </div>

          <ScrollRestoration scopeId={section} className="demo-scroll-area">
            {section === "overview" && (
              <OverviewSection onOpenModal={openModal} onConfirm={confirm} />
            )}
            {section === "library" && <LibrarySection onConfirm={confirm} />}
            {section === "settings" && <SettingsSection onConfirm={confirm} />}
          </ScrollRestoration>
        </main>
      </FocusScope>

      <footer className="demo-footer">
        <span>
          Last confirmed: <strong>{confirmedAction}</strong>
        </span>
        <span>
          Arrows / WASD to move · Enter / Space to confirm · Esc to go back
        </span>
      </footer>

      {modalOpen && (
        <div className="modal-backdrop">
          <NavigationDialog
            scopeId="demo-modal"
            initialFocusId="modal-primary"
            className="demo-modal"
            onBack={() => {
              setModalOpen(false);
              return true;
            }}
          >
            <p className="eyebrow">Modal scope</p>
            <h2>Focus stays inside.</h2>
            <p>
              Este diálogo pausa el scope inferior y restaura exactamente el
              elemento que lo abrió al cerrarse.
            </p>
            <div className="modal-actions">
              <Focusable
                focusId="modal-primary"
                scopeId="demo-modal"
                className="action-card primary"
                onConfirm={() => {
                  confirm("Modal primary");
                  setModalOpen(false);
                }}
              >
                <span>Primary action</span>
              </Focusable>
              <Focusable
                focusId="modal-secondary"
                scopeId="demo-modal"
                className="action-card"
                onConfirm={() => confirm("Modal secondary")}
              >
                <span>Secondary action</span>
              </Focusable>
              <Focusable
                focusId="modal-close"
                scopeId="demo-modal"
                className="action-card quiet"
                onConfirm={() => setModalOpen(false)}
              >
                <span>Close dialog</span>
              </Focusable>
            </div>
          </NavigationDialog>
        </div>
      )}

      <NavigationDebugOverlay />
    </div>
  );
}

interface DemoActionsProps {
  onConfirm: (message: string) => void;
}

function OverviewSection({
  onOpenModal,
  onConfirm,
}: DemoActionsProps & { onOpenModal: () => void }) {
  return (
    <div className="section-stack">
      <section className="demo-panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Horizontal row</p>
            <h3>Quick actions</h3>
          </div>
          <span className="panel-meta">5 items</span>
        </div>
        <NavigationRow>
          {["Resume", "Discover", "Queue", "Stats"].map((label, index) => (
            <Focusable
              key={label}
              focusId={`overview-action-${index}`}
              scopeId="demo-app"
              className="action-card"
              onConfirm={() => onConfirm(label)}
            >
              <span>{label}</span>
            </Focusable>
          ))}
          <Focusable
            focusId="overview-open-modal"
            scopeId="demo-app"
            className="action-card accent"
            onConfirm={onOpenModal}
          >
            <span>Open modal</span>
          </Focusable>
        </NavigationRow>
      </section>

      <section className="demo-panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Vertical list</p>
            <h3>Disabled elements are skipped</h3>
          </div>
          <span className="panel-meta">1 disabled</span>
        </div>
        <NavigationList>
          {[
            "Continue session",
            "Open details",
            "Disabled item",
            "Preferences",
            "Sign out",
          ].map((label, index) => (
            <Focusable
              key={label}
              focusId={`overview-list-${index}`}
              scopeId="demo-app"
              className="list-row"
              disabled={label === "Disabled item"}
              onConfirm={() => onConfirm(label)}
            >
              <span>{label}</span>
              <span className="list-arrow">↗</span>
            </Focusable>
          ))}
        </NavigationList>
      </section>
    </div>
  );
}

function LibrarySection({ onConfirm }: DemoActionsProps) {
  return (
    <section className="demo-panel">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Spatial navigation</p>
          <h3>30 candidates, one grid</h3>
        </div>
        <span className="panel-meta">5 × 6</span>
      </div>
      <NavigationGrid columns={5}>
        {Array.from({ length: 30 }, (_, index) => {
          const disabled = index % 9 === 0;
          return (
            <Focusable
              key={index}
              focusId={`library-item-${index}`}
              scopeId="demo-app"
              className="grid-card"
              disabled={disabled}
              onConfirm={() => onConfirm(`Grid item ${index + 1}`)}
            >
              <span className="grid-index">
                {String(index + 1).padStart(2, "0")}
              </span>
              <span>{disabled ? "Unavailable" : "Candidate"}</span>
            </Focusable>
          );
        })}
      </NavigationGrid>
    </section>
  );
}

function SettingsSection({ onConfirm }: DemoActionsProps) {
  return (
    <section className="demo-panel settings-panel">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Reusable primitives</p>
          <h3>Settings placeholder</h3>
        </div>
        <span className="panel-meta">No real settings yet</span>
      </div>
      <NavigationList>
        {["Input preferences", "Accessibility", "Motion", "About LumaDeck"].map(
          (label, index) => (
            <Focusable
              key={label}
              focusId={`settings-action-${index}`}
              scopeId="demo-app"
              className="list-row"
              onConfirm={() => onConfirm(label)}
            >
              <span>{label}</span>
              <span className="list-arrow">→</span>
            </Focusable>
          ),
        )}
      </NavigationList>
    </section>
  );
}
