import { Component, type ErrorInfo, type ReactNode } from "react";
import { bluetoothService } from "./bluetooth-service";

interface BluetoothErrorBoundaryProps {
  children: ReactNode;
}

interface BluetoothErrorBoundaryState {
  error: Error | null;
}

export class BluetoothErrorBoundary extends Component<
  BluetoothErrorBoundaryProps,
  BluetoothErrorBoundaryState
> {
  state: BluetoothErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): BluetoothErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    void bluetoothService
      .recordClientDiagnostic(
        "error",
        "ui.render_error",
        `message=${error.message}; componentStack=${info.componentStack ?? "<none>"}`,
      )
      .catch(() => undefined);
  }

  render() {
    if (!this.state.error) return this.props.children;

    return (
      <article className="bluetooth-empty-panel" role="alert">
        <p className="eyebrow">Bluetooth</p>
        <h2>No se pudo cargar esta pantalla.</h2>
        <p>
          Se registró el error para poder diagnosticarlo. Puedes reintentar sin
          reiniciar LumaDeck.
        </p>
        <button
          className="settings-button primary"
          type="button"
          onClick={() => this.setState({ error: null })}
        >
          Reintentar
        </button>
      </article>
    );
  }
}
