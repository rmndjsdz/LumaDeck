import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { DetailsTabContent } from "./DetailsTabContent";

describe("DetailsTabContent", () => {
  let host: HTMLDivElement | undefined;
  let root: Root | undefined;

  afterEach(async () => {
    await act(async () => {
      root?.unmount();
    });
    root = undefined;
    host?.remove();
    host = undefined;
  });

  it("keeps the transition container when the active section changes", async () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);

    function Fixture() {
      const [section, setSection] = useState<"summary" | "activity">("summary");
      return (
        <>
          <button type="button" onClick={() => setSection("activity")}>
            Activity
          </button>
          <DetailsTabContent
            activeSection={section}
            direction={section === "summary" ? "forward" : "backward"}
          >
            <span data-testid="section-content">{section}</span>
          </DetailsTabContent>
        </>
      );
    }

    await act(async () => {
      root?.render(<Fixture />);
    });
    const container = host.querySelector(".details-tab-content");

    await act(async () => {
      host
        ?.querySelector("button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(host.querySelector(".details-tab-content")).toBe(container);
    expect(container?.getAttribute("data-active-section")).toBe("activity");
    expect(container?.getAttribute("data-transition-direction")).toBe(
      "backward",
    );
    expect(
      host.querySelector("[data-testid=section-content]")?.textContent,
    ).toBe("activity");
  });
});
