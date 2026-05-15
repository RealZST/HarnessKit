import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./lib/i18n";

// biome-ignore lint/style/noNonNullAssertion: standard React entry — index.html guarantees a #root mount point.
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
