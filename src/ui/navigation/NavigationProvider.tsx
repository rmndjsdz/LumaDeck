import { useEffect, useMemo, type PropsWithChildren } from "react";

import { useNavigationStore } from "../../stores/navigation-store";
import { FocusRegistry } from "./core/focus-registry";
import { NavigationEngine } from "./core/navigation-engine";
import { InputManager } from "./input/input-manager";
import { FocusScrollManager } from "./scroll/focus-scroll-manager";
import { NavigationContext } from "./navigation-context";

export interface NavigationRuntime {
  registry: FocusRegistry;
  engine: NavigationEngine;
  inputManager: InputManager;
  scrollManager: FocusScrollManager;
}

export function NavigationProvider({ children }: PropsWithChildren) {
  const runtime = useMemo(() => {
    const registry = new FocusRegistry();
    const scrollManager = new FocusScrollManager();
    const engine = new NavigationEngine(registry, scrollManager);
    return {
      registry,
      engine,
      inputManager: new InputManager(engine),
      scrollManager,
    } satisfies NavigationRuntime;
  }, []);
  const inputMode = useNavigationStore((state) => state.inputMode);

  useEffect(() => {
    runtime.inputManager.start();
    const invalidate = () => runtime.registry.invalidateAll();
    window.addEventListener("resize", invalidate);
    window.addEventListener("scroll", invalidate, true);
    const unsubscribe = runtime.registry.subscribe(() => {
      useNavigationStore.getState().updateDebug({
        registryCount: runtime.registry.count(),
        duplicateFocusIds: runtime.registry.getDuplicateIds(),
      });
    });
    useNavigationStore.getState().updateDebug({
      registryCount: runtime.registry.count(),
      duplicateFocusIds: runtime.registry.getDuplicateIds(),
    });
    return () => {
      unsubscribe();
      window.removeEventListener("resize", invalidate);
      window.removeEventListener("scroll", invalidate, true);
      runtime.inputManager.dispose();
    };
  }, [runtime]);

  useEffect(() => {
    document.documentElement.dataset.inputMode = inputMode;
  }, [inputMode]);

  return (
    <NavigationContext.Provider value={runtime}>
      <div className="navigation-root" data-input-mode={inputMode}>
        {children}
      </div>
    </NavigationContext.Provider>
  );
}
