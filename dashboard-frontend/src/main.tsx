import React from "react";
import { installPlatformErrorRecorder, installPlatformFetchRecorder, installPlatformLifecycleRecorder, installPlatformNavigationRecorder } from "./platformRecorder";
import ReactDOM from "react-dom/client";
import { PlatformErrorBoundary } from "./PlatformErrorBoundary";
import App from "./App";
import "leaflet/dist/leaflet.css";
import "./styles.css";

installPlatformErrorRecorder();
installPlatformFetchRecorder();
installPlatformLifecycleRecorder();
installPlatformNavigationRecorder();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <PlatformErrorBoundary>
      <App />
    </PlatformErrorBoundary>
  </React.StrictMode>,
);







