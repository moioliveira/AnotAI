/**
 * Ponte com o núcleo Rust (comandos e eventos).
 * Espelha os tipos do Rust à mão por enquanto; tauri-specta vai gerá-los
 * (Tech Spec 5.4).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AppState = "idle" | "listening" | "transcribing" | "inserting" | "error";
export type Mode = "dictation" | "translation";
export type CloudProvider = "openAi" | "groq" | "deepgram";
export type KeyProvider = "openai" | "groq" | "deepgram" | "anthropic";
export type InsertMethod = "auto" | "type" | "paste";
export type Theme = "light" | "dark" | "system";

export type CoreEvent =
  | { type: "state-changed"; payload: { state: AppState; mode: Mode } }
  | { type: "audio-level"; payload: { rms: number } }
  | { type: "transcript-partial"; payload: { text: string } }
  | { type: "transcript-final"; payload: { text: string } }
  | { type: "history-changed" };

export interface Settings {
  hotkey: { alternate: string; doubleTapMs: number; holdThresholdMs: number };
  engine: { mode: EngineMode; cloudProvider: CloudProvider; localModel: string };
  postproc: { correction: boolean; removeFillers: boolean; llmModel: string };
  mode: Mode;
  insertMethod: InsertMethod;
  history: { retentionDays: number };
  saveRecordings: boolean;
  theme: Theme;
  launchAtStartup: boolean;
  telemetry: boolean;
  /** Consentimento LGPD para a nuvem (epoch ms) ou null. */
  cloudConsentAt: number | null;
  onboardingDone: boolean;
}

export type EngineMode = "auto" | "cloud" | "local";

export interface ModelStatus {
  id: string;
  label: string;
  description: string;
  size: number;
  installed: boolean;
  downloading: boolean;
}

export interface ModelProgress {
  id: string;
  received: number;
  total: number;
}

export interface HotkeyStatus {
  ctrlWinError: string | null;
  alternate: string | null;
  alternateError: string | null;
}

export interface AppInfo {
  version: string;
  hotkey: HotkeyStatus;
  dataDir: string | null;
}

export interface Transcription {
  id: string;
  createdAt: number;
  mode: Mode;
  trigger: "hold" | "toggle";
  rawText: string;
  finalText: string;
  sourceLang: string;
  targetLang: string;
  targetApp: string | null;
  inserted: boolean;
  durationMs: number;
  latencyMs: number;
  engine: string;
}

export interface DictionaryTerm {
  id: string;
  term: string;
  spokenAs: string | null;
  createdAt: number;
}

export interface DailyStat {
  day: string;
  words: number;
  sessions: number;
  seconds: number;
}

export const CORE_EVENT = "core-event";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (next: Settings) => invoke<Settings>("save_settings", { new: next }),
  setMode: (mode: Mode) => invoke<void>("set_mode", { mode }),
  apiKeyStatus: () => invoke<Record<KeyProvider, boolean>>("api_key_status"),
  setApiKey: (provider: KeyProvider, key: string) => invoke<void>("set_api_key", { provider, key }),
  history: (query?: string, limit?: number) =>
    invoke<Transcription[]>("history_list", { query, limit }),
  deleteHistoryItem: (id: string) => invoke<void>("history_delete", { id }),
  deleteAllData: () => invoke<void>("delete_all_data"),
  exportData: () => invoke<string>("export_data"),
  dictionary: () => invoke<DictionaryTerm[]>("dictionary_list"),
  addTerm: (term: string, spokenAs?: string) =>
    invoke<DictionaryTerm>("dictionary_add", { term, spokenAs: spokenAs || null }),
  removeTerm: (id: string) => invoke<void>("dictionary_remove", { id }),
  stats: (days = 30) => invoke<DailyStat[]>("stats", { days }),
  modelsStatus: () => invoke<ModelStatus[]>("models_status"),
  downloadModel: (id: string) => invoke<void>("model_download", { id }),
  deleteModel: (id: string) => invoke<void>("model_delete", { id }),
  micTest: (on: boolean) => invoke<void>("mic_test", { on }),
  setCloudConsent: (granted: boolean) => invoke<Settings>("set_cloud_consent", { granted }),
};

export function onModelProgress(handler: (p: ModelProgress) => void): Promise<UnlistenFn> {
  return listen<ModelProgress>("model-progress", (e) => handler(e.payload));
}

/** O que falta para ditar com as configurações atuais (vazio = pronto). */
export function setupGaps(
  s: Settings,
  keys: Record<KeyProvider, boolean>,
  models: ModelStatus[],
): string[] {
  const providerKey: Record<CloudProvider, KeyProvider> = {
    openAi: "openai",
    groq: "groq",
    deepgram: "deepgram",
  };
  const cloudReady = s.cloudConsentAt !== null && keys[providerKey[s.engine.cloudProvider]];
  const localReady = models.some((m) => m.id === s.engine.localModel && m.installed);
  const gaps: string[] = [];
  if (s.engine.mode === "cloud" && !cloudReady) {
    gaps.push(
      s.cloudConsentAt === null
        ? "O motor é Nuvem, mas o processamento na nuvem não foi autorizado."
        : "Falta a chave de API do provedor de transcrição.",
    );
  }
  if (s.engine.mode === "local" && !localReady) gaps.push("O modelo local ainda não foi baixado.");
  if (s.engine.mode === "auto" && !cloudReady && !localReady) {
    gaps.push("Configure a nuvem (autorização e chave) ou baixe um modelo local.");
  }
  if (s.postproc.correction && (s.cloudConsentAt === null || !keys.anthropic)) {
    gaps.push(
      "Sem a chave da Anthropic (e a autorização de nuvem), o texto sai só com as regras básicas: sem correção pela norma culta nem tradução.",
    );
  }
  return gaps;
}

export function formatMb(bytes: number): string {
  return `${Math.round(bytes / 1_000_000)} MB`;
}

export function onCoreEvent(handler: (event: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>(CORE_EVENT, (e) => handler(e.payload));
}

/** Mensagem de erro de um `invoke` (o Rust devolve texto). */
export function errorText(err: unknown): string {
  return typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
}
