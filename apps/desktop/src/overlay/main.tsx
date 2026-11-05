import { messages } from "@anotai/i18n";
import { motion } from "motion/react";
import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { onCoreEvent } from "../lib/core";
import { useCoreStore } from "../lib/store";
import "../styles.css";

const t = messages();

/** Pílula flutuante: aparece enquanto ouve/transcreve. Janela não recebe foco. */
function Overlay() {
  const { state, mode, level, partial, apply } = useCoreStore();

  useEffect(() => {
    const unlisten = onCoreEvent(apply);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [apply]);

  const accent = mode === "translation" ? "bg-translation" : "bg-recording";

  return (
    <div className="flex h-screen items-end justify-center pb-2">
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        className="flex items-center gap-3 rounded-full bg-night/90 px-4 py-2 text-white shadow-lg"
      >
        <motion.span
          className={`size-2.5 rounded-full ${accent}`}
          animate={{ scale: 1 + Math.min(level * 4, 1.2) }}
        />
        <span className="max-w-72 truncate text-sm">{partial || t.state[state]}</span>
      </motion.div>
    </div>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("#root não encontrado");

createRoot(root).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
