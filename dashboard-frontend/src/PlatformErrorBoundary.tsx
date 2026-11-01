import React from "react";
import { recordReactError } from "./platformRecorder";

type Props = {
  children: React.ReactNode;
};

type State = {
  hasError: boolean;
};

export class PlatformErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo): void {
    recordReactError(error, info.componentStack ?? undefined);
  }

  render(): React.ReactNode {
    if (this.state.hasError) {
      return (
        <div style={{ padding: 24 }}>
          <h2>Dashboard error</h2>
          <p>The error was recorded for diagnostics and replay.</p>
        </div>
      );
    }

    return this.props.children;
  }
}
