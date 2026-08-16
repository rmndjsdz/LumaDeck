import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { ProductShell } from "./features/launcher/ProductShell";
import { NavigationProvider } from "./ui/navigation/NavigationProvider";
import { AutoCursor } from "./ui/input/AutoCursor";
import { recordMediaTiming } from "./ui/performance/media-timing";
import { ThemeProvider } from "./ui/theme/ThemeProvider";
import "./App.css";

function App() {
  const [queryClient] = useState(() => new QueryClient());
  return (
    <QueryClientProvider client={queryClient}>
      <DetailsQueryDiagnostics queryClient={queryClient} />
      <ThemeProvider>
        <NavigationProvider>
          <AutoCursor />
          <ProductShell />
        </NavigationProvider>
      </ThemeProvider>
    </QueryClientProvider>
  );
}

function DetailsQueryDiagnostics({
  queryClient,
}: {
  queryClient: QueryClient;
}) {
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    return queryClient.getQueryCache().subscribe((event) => {
      const queryKey = event.query.queryKey;
      if (queryKey[0] !== "game-details" || typeof queryKey[1] !== "string") {
        return;
      }
      const action = event.type === "updated" ? event.action.type : event.type;
      recordMediaTiming("DETAILS_QUERY_EVENT", {
        gameId: queryKey[1],
        type: "screenshot",
        detail: JSON.stringify({
          event: event.type,
          action,
          status: event.query.state.status,
          fetchStatus: event.query.state.fetchStatus,
          dataPresent: Boolean(event.query.state.data),
          dataUpdatedAt: event.query.state.dataUpdatedAt,
          isInvalidated: event.query.state.isInvalidated,
          isStale: event.query.isStale(),
          gcTime: event.query.gcTime,
          observers: event.query.getObserversCount(),
        }),
      });
    });
  }, [queryClient]);
  return null;
}

export default App;
