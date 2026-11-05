import { beforeEach, describe, expect, it } from "vitest";
import { useCoreStore } from "./store";

describe("useCoreStore", () => {
  beforeEach(() => {
    useCoreStore.setState({
      state: "idle",
      mode: "dictation",
      partial: "",
      lastFinal: "",
      historyVersion: 0,
    });
  });

  it("aplica mudança de estado e modo", () => {
    useCoreStore.getState().apply({
      type: "state-changed",
      payload: { state: "listening", mode: "translation" },
    });
    expect(useCoreStore.getState()).toMatchObject({ state: "listening", mode: "translation" });
  });

  it("transcrição final limpa a parcial", () => {
    const { apply } = useCoreStore.getState();
    apply({ type: "transcript-partial", payload: { text: "olá mun" } });
    apply({ type: "transcript-final", payload: { text: "Olá, mundo." } });
    expect(useCoreStore.getState()).toMatchObject({ partial: "", lastFinal: "Olá, mundo." });
  });

  it("novo ditado zera o texto parcial", () => {
    const { apply } = useCoreStore.getState();
    apply({ type: "transcript-partial", payload: { text: "antigo" } });
    apply({ type: "state-changed", payload: { state: "listening", mode: "dictation" } });
    expect(useCoreStore.getState().partial).toBe("");
  });

  it("history-changed incrementa a versão", () => {
    useCoreStore.getState().apply({ type: "history-changed" });
    useCoreStore.getState().apply({ type: "history-changed" });
    expect(useCoreStore.getState().historyVersion).toBe(2);
  });
});
