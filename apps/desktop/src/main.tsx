import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, applyTheme } from "./App";
import { api } from "./lib/core";
import "./styles.css";

// Tema do sistema até as configurações carregarem.
applyTheme("system");
api
  .getSettings()
  .then((s) => {
    applyTheme(s.theme);
    window
      .matchMedia("(prefers-color-scheme: dark)")
      .addEventListener("change", () => applyTheme(s.theme));
  })
  .catch(console.error);

const root = document.getElementById("root");
if (!root) throw new Error("#root não encontrado");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
