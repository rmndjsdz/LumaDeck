import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { ProductShell } from "./features/launcher/ProductShell";
import { NavigationProvider } from "./ui/navigation/NavigationProvider";
import { AutoCursor } from "./ui/input/AutoCursor";
import "./App.css";

function App() {
  const [queryClient] = useState(() => new QueryClient());
  return (
    <QueryClientProvider client={queryClient}>
      <NavigationProvider>
        <AutoCursor />
        <ProductShell />
      </NavigationProvider>
    </QueryClientProvider>
  );
}

export default App;
