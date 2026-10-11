import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// This guest integration is stripped out of normal production bundles.
// It is enabled only for the opt-in synthetic-data native E2E build.
if (import.meta.env.VITE_NATIVE_E2E === "1") {
  await import("@wdio/tauri-plugin");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
