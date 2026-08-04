import { describe, expect, it } from "vitest";
import { useNavigationStore } from "../../../stores/navigation-store";
import { FocusRegistry } from "../core/focus-registry";
import { NavigationEngine } from "../core/navigation-engine";
import { FocusScrollManager } from "../scroll/focus-scroll-manager";
import { InputManager } from "./input-manager";

function addElement(): HTMLElement {
  const element = document.createElement("button");
  document.body.appendChild(element);
  element.getBoundingClientRect = () => new DOMRect(0, 0, 80, 40);
  return element;
}

describe("InputManager session freeze", () => {
  it("does not forward keyboard, pointer, or gamepad actions while frozen", () => {
    useNavigationStore.setState({
      activeScopeId: null,
      activeFocusId: null,
      previousFocusId: null,
      lastNavigationAction: null,
    });
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "target",
      scopeId: "root",
      element: addElement(),
    });
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "target",
    });
    const inputManager = new InputManager(engine);
    inputManager.setInputFrozen(true);
    const focusBeforeFreezeInput = engine.getActiveFocusId();

    expect(inputManager.dispatch("move-right", "keyboard")).toBe(true);
    inputManager.handlePointerConfirm("target");
    expect(engine.getActiveFocusId()).toBe(focusBeforeFreezeInput);

    inputManager.dispose();
    engine.dispose();
  });
});
