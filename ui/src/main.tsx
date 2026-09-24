import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";
import { applyDensity, storedDensity } from "./lib/density";

// Before the first render, so the interface never appears at the wrong size.
applyDensity(storedDensity());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
