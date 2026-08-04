import { describe, expect, it } from "vitest";

import { useNavigationStore } from "../../../stores/navigation-store";
import { FocusScrollManager } from "../scroll/focus-scroll-manager";
import { FocusRegistry } from "./focus-registry";
import { NavigationEngine } from "./navigation-engine";

describe("scope action priority", () => {
  it("lets a modal consume trigger actions before the global route handler", () => {
    useNavigationStore.setState({
      activeScopeId: null,
      activeFocusId: null,
      previousFocusId: null,
      lastNavigationAction: null,
    });
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const root = document.createElement("button");
    const modal = document.createElement("button");
    document.body.append(root, modal);
    registry.register({
      focusId: "root",
      scopeId: "root",
      element: root,
    });
    registry.register({
      focusId: "modal",
      scopeId: "modal",
      element: modal,
    });
    const actions: string[] = [];
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "root",
      activateOnMount: true,
    });
    engine.registerScope({
      scopeId: "modal",
      parentScopeId: "root",
      initialFocusId: "modal",
      modal: true,
      onAction: (action) => {
        if (action === "previous-primary-screen") {
          actions.push(action);
          return true;
        }
        return false;
      },
    });
    engine.prepareScopeOpen("modal", "root");
    engine.activateScope("modal", "modal");

    expect(engine.dispatch("previous-primary-screen", "gamepad")).toBe(true);
    expect(actions).toEqual(["previous-primary-screen"]);
    expect(engine.getPrimaryNavigationBlockReason()).toBe("modal");
    engine.dispose();
  });
});
