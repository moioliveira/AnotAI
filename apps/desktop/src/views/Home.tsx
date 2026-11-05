import { messages } from "@anotai/i18n";
import { Button, Card, cn } from "@anotai/ui";
import { AlertTriangle, Keyboard, Languages, Mic } from "lucide-react";
import { useEffect, useState } from "react";
import {
  type AppInfo,
  api,
  type DailyStat,
  errorText,
  type KeyProvider,
  type Mode,
  type ModelStatus,
  type Settings,
  setupGaps,
} from "../lib/core";
import { useCoreStore } from "../lib/store";

const t = messages();

export function Home({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { state, mode, lastFinal, partial, historyVersion, setMode } = useCoreStore();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [keys, setKeys] = useState<Record<KeyProvider, boolean> | null>(null);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [today, setToday] = useState<DailyStat | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.appInfo(), api.getSettings(), api.apiKeyStatus(), api.modelsStatus()])
      .then(([i, s, k, m]) => {
        setInfo(i);
        setSettings(s);
        setKeys(k);
        setModels(m);
        setMode(s.mode);
      })
      .catch((e) => setError(errorText(e)));
  }, [setMode]);

  useEffect(() => {
    void historyVersion;
    api
      .stats(0)
      .then((s) => setToday(s[0] ?? null))
      .catch(() => setToday(null));
  }, [historyVersion]);

  const changeMode = (next: Mode) => {
    setMode(next);
    api.setMode(next).catch((e) => setError(errorText(e)));
  };

  const warnings: string[] = settings && keys ? setupGaps(settings, keys, models) : [];
  if (info?.hotkey.ctrlWinError)
    warnings.push(`Ctrl + Win indisponível: ${info.hotkey.ctrlWinError}`);
  if (info?.hotkey.alternateError) warnings.push(info.hotkey.alternateError);

  const listening = state === "listening";
  const accent =
    mode === "translation" ? "bg-translation" : listening ? "bg-recording" : "bg-brand";

  return (
    <div className="flex flex-col gap-6">
      <header>
        <h1 className="font-display font-semibold text-2xl">{t.app.name}</h1>
        <p className="text-muted-foreground text-sm">{t.app.tagline}</p>
      </header>

      {error && <p className="rounded-md bg-recording/10 p-3 text-sm">{error}</p>}

      {warnings.length > 0 && (
        <Card className="border-recording/40">
          <div className="flex gap-3">
            <AlertTriangle className="mt-0.5 size-4 shrink-0 text-recording" />
            <div className="flex flex-col gap-2 text-sm">
              {warnings.map((w) => (
                <p key={w}>{w}</p>
              ))}
              <Button size="sm" variant="outline" className="self-start" onClick={onOpenSettings}>
                Abrir configurações
              </Button>
            </div>
          </div>
        </Card>
      )}

      <Card>
        <div className="flex items-center gap-3">
          <span className={cn("size-3 rounded-full", accent, listening && "animate-pulse")} />
          <span className="font-medium" aria-live="polite">
            {t.state[state]}
          </span>
          <fieldset className="ml-auto flex gap-2">
            <legend className="sr-only">Modo</legend>
            <Button
              size="sm"
              variant={mode === "dictation" ? "default" : "outline"}
              aria-pressed={mode === "dictation"}
              onClick={() => changeMode("dictation")}
            >
              <Mic /> {t.mode.dictation}
            </Button>
            <Button
              size="sm"
              variant={mode === "translation" ? "default" : "outline"}
              aria-pressed={mode === "translation"}
              onClick={() => changeMode("translation")}
            >
              <Languages /> {t.mode.translation}
            </Button>
          </fieldset>
        </div>
        <ul className="mt-5 space-y-2 text-muted-foreground text-sm">
          <li className="flex items-center gap-2">
            <Mic className="size-4" /> {t.hint.hold}
            {info?.hotkey.alternate && !info.hotkey.alternateError && (
              <span>(ou {info.hotkey.alternate})</span>
            )}
          </li>
          <li className="flex items-center gap-2">
            <Keyboard className="size-4" /> {t.hint.doubleTap}
          </li>
        </ul>
        {(partial || lastFinal) && (
          <p className="mt-5 whitespace-pre-wrap rounded-md bg-muted p-3 font-mono text-sm">
            {partial || lastFinal}
          </p>
        )}
      </Card>

      <div className="grid grid-cols-3 gap-4">
        <Stat label="Palavras hoje" value={today?.words ?? 0} />
        <Stat label="Ditados hoje" value={today?.sessions ?? 0} />
        <Stat label="Minutos falados" value={Math.round((today?.seconds ?? 0) / 60)} />
      </div>

      <footer className="text-muted-foreground text-xs">v{info?.version ?? "…"}</footer>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <Card className="p-4">
      <p className="text-muted-foreground text-xs">{label}</p>
      <p className="font-display font-semibold text-2xl tabular-nums">{value}</p>
    </Card>
  );
}
