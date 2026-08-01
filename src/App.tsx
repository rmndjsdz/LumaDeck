import { NavigationDemo } from "./features/navigation-demo/NavigationDemo";
import { NavigationProvider } from "./ui/navigation/NavigationProvider";
import "./App.css";

function App() {
  return (
    <NavigationProvider>
      <NavigationDemo />
    </NavigationProvider>
  );
}

export default App;
