import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

function mount() {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

// Only a build made for Ticket 17's measurements holds the probe; every other
// build compiles this branch away.
if (import.meta.env.VITE_TASKNOTES_PERF_PROBE === "1") {
  void import("./qa/perf-probe").then((probe) => {
    probe.prepare();
    mount();
    void probe.run();
  });
} else {
  mount();
}
