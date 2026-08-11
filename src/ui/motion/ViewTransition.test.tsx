import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { useProductStore } from "../../stores/product-store";
import { ViewTransition } from "./ViewTransition";

describe("ViewTransition", () => {
  let host: HTMLDivElement | undefined;

  afterEach(() => {
    host?.remove();
    host = undefined;
    useProductStore.setState({ activeView: "home", viewTransitionId: 0 });
  });

  it("does not remount the view tree when the transition id changes", async () => {
    host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);

    await act(async () => {
      root.render(
        <ViewTransition view="home">
          <span data-testid="stable-child" />
        </ViewTransition>,
      );
    });
    const child = host.querySelector("[data-testid=stable-child]");

    await act(async () => {
      useProductStore.setState({ viewTransitionId: 1 });
    });

    expect(host.querySelector("[data-testid=stable-child]")).toBe(child);
    await act(async () => root.unmount());
  });
});
