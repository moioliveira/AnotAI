import { create } from "zustand";
import type { AppState, CoreEvent, Mode } from "./core";

interface CoreStore {
  state: AppState;
  mode: Mode;
  level: number;
  partial: string;
  lastFinal: string;
  /** Muda a cada `history-changed`: telas recarregam ao observar. */
  historyVersion: number;
  apply: (event: CoreEvent) => void;
  setMode: (mode: Mode) => void;
}

export const useCoreStore = create<CoreStore>((set) => ({
  state: "idle",
  mode: "dictation",
  level: 0,
  partial: "",
  lastFinal: "",
  historyVersion: 0,
  setMode: (mode) => set({ mode }),
  apply: (event) => {
    switch (event.type) {
      case "state-changed":
        set((s) => ({
          state: event.payload.state,
          mode: event.payload.mode,
          // Um ditado novo começa sem texto parcial.
          partial: event.payload.state === "listening" ? "" : s.partial,
        }));
        break;
      case "audio-level":
        set({ level: event.payload.rms });
        break;
      case "transcript-partial":
        set({ partial: event.payload.text });
        break;
      case "transcript-final":
        set({ lastFinal: event.payload.text, partial: "" });
        break;
      case "history-changed":
        set((s) => ({ historyVersion: s.historyVersion + 1 }));
        break;
    }
  },
}));
