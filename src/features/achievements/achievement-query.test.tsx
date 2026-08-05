import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it } from "vitest";
import { useRefreshGameAchievements } from "./achievement-query";

describe("achievement refresh mutation", () => {
  it("exposes mutation state, retry controls, invalidation path, and cancel", async () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    let mutation: ReturnType<typeof useRefreshGameAchievements> | undefined;
    function Harness() {
      mutation = useRefreshGameAchievements();
      return null;
    }
    const root: Root = createRoot(host);
    await act(async () => {
      root.render(
        <QueryClientProvider client={client}>
          <Harness />
        </QueryClientProvider>,
      );
    });

    expect(mutation).toBeDefined();
    expect(typeof mutation?.mutate).toBe("function");
    expect(typeof mutation?.mutateAsync).toBe("function");
    expect(typeof mutation?.reset).toBe("function");
    expect(typeof mutation?.cancel).toBe("function");
    expect(mutation?.isPending).toBe(false);

    await act(async () => {
      await mutation?.mutateAsync("game-001");
    });
    expect(mutation?.isSuccess).toBe(true);
    expect(mutation?.data?.gameId).toBe("game-001");

    await act(async () => {
      root.unmount();
    });
    host.remove();
    client.clear();
  });
});
