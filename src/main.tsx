import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { api } from "./lib/api";
import "./index.css";

// Gli errori del frontend finiscono nello stesso log del backend.
window.addEventListener("error", (e) => {
  void api.log("error", `${e.message} (${e.filename}:${e.lineno})`);
});
window.addEventListener("unhandledrejection", (e) => {
  void api.log("error", `promessa rifiutata: ${String(e.reason)}`);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
