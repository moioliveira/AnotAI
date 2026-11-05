import { Button, Card, Field, Input, Select, Switch } from "@anotai/ui";
import { CheckCircle2, CircleDashed } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { applyTheme } from "../App";
import { ModelsPanel } from "../components/ModelsPanel";
import {
  type AppInfo,
  api,
  type CloudProvider,
  type EngineMode,
  errorText,
  type InsertMethod,
  type KeyProvider,
  type Settings,
  type Theme,
} from "../lib/core";

const providers: { id: CloudProvider; key: KeyProvider; name: string; model: string }[] = [
  { id: "groq", key: "groq", name: "Groq", model: "Whisper large-v3-turbo" },
  { id: "openAi", key: "openai", name: "OpenAI", model: "gpt-4o-transcribe" },
  { id: "deepgram", key: "deepgram", name: "Deepgram", model: "Nova-3" },
];

const keyLabels: Record<KeyProvider, string> = {
  groq: "Groq",
  openai: "OpenAI",
  deepgram: "Deepgram",
  anthropic: "Anthropic (correção e tradução)",
};

export function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [keys, setKeys] = useState<Record<KeyProvider, boolean> | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.getSettings(), api.appInfo(), api.apiKeyStatus()])
      .then(([s, i, k]) => {
        setSettings(s);
        setInfo(i);
        setKeys(k);
      })
      .catch((e) => setMessage(errorText(e)));
  }, []);

  if (!settings) return <p className="text-muted-foreground text-sm">{message ?? "Carregando…"}</p>;

  /** Salva já (autosave); em erro, volta ao valor anterior. */
  const update = async (patch: (s: Settings) => Settings) => {
    const previous = settings;
    const next = patch(structuredClone(settings));
    setSettings(next);
    try {
      await api.saveSettings(next);
      if (next.theme !== previous.theme) applyTheme(next.theme);
      if (JSON.stringify(next.hotkey) !== JSON.stringify(previous.hotkey)) {
        setInfo(await api.appInfo());
      }
      setMessage(null);
    } catch (e) {
      setSettings(previous);
      setMessage(errorText(e));
    }
  };

  const saveKey = async (provider: KeyProvider, value: string) => {
    try {
      await api.setApiKey(provider, value);
      setKeys(await api.apiKeyStatus());
      setMessage(value ? `Chave de ${keyLabels[provider]} salva.` : "Chave removida.");
    } catch (e) {
      setMessage(errorText(e));
    }
  };

  return (
    <div className="flex flex-col gap-5">
      <header>
        <h1 className="font-display font-semibold text-2xl">Configurações</h1>
        <p className="text-muted-foreground text-sm">As mudanças valem na hora.</p>
      </header>

      {message && (
        <p className="rounded-md bg-muted p-3 text-sm" role="status">
          {message}
        </p>
      )}

      <Section title="Transcrição">
        <Field label="Provedor" htmlFor="provider" hint="Provisório até o benchmark da Sprint 0.">
          <Select
            id="provider"
            value={settings.engine.cloudProvider}
            onChange={(e) =>
              update((s) => ({
                ...s,
                engine: { ...s.engine, cloudProvider: e.target.value as CloudProvider },
              }))
            }
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} — {p.model}
              </option>
            ))}
          </Select>
        </Field>
        <Field
          label="Motor"
          htmlFor="engine-mode"
          hint="Automático usa a nuvem e cai para o modelo local se a rede ou o provedor falhar."
        >
          <Select
            id="engine-mode"
            value={settings.engine.mode}
            onChange={(e) =>
              update((s) => ({ ...s, engine: { ...s.engine, mode: e.target.value as EngineMode } }))
            }
          >
            <option value="auto">Automático (nuvem + local)</option>
            <option value="cloud">Só nuvem</option>
            <option value="local">Só local (o áudio não sai do computador)</option>
          </Select>
        </Field>
        <Field label="Modelo local (whisper.cpp)">
          <ModelsPanel
            selected={settings.engine.localModel}
            onSelect={(id) => update((s) => ({ ...s, engine: { ...s.engine, localModel: id } }))}
          />
        </Field>
      </Section>

      <Section
        title="Chaves de API"
        hint="Guardadas no Gerenciador de Credenciais do Windows. Nunca são exibidas de volta."
      >
        {(Object.keys(keyLabels) as KeyProvider[]).map((p) => (
          <KeyField
            key={p}
            provider={p}
            label={keyLabels[p]}
            saved={keys?.[p] ?? false}
            onSave={saveKey}
          />
        ))}
      </Section>

      <Section title="Texto">
        <Field
          inline
          label="Corrigir pela norma culta"
          hint="Pontuação, acentuação e concordância com o Claude. Precisa da chave da Anthropic."
        >
          <Switch
            label="Corrigir pela norma culta"
            checked={settings.postproc.correction}
            onCheckedChange={(v) =>
              update((s) => ({ ...s, postproc: { ...s.postproc, correction: v } }))
            }
          />
        </Field>
        <Field
          inline
          label="Remover hesitações"
          hint="“éé”, “hã”, “tipo assim”, “né” no fim da frase."
        >
          <Switch
            label="Remover hesitações"
            checked={settings.postproc.removeFillers}
            onCheckedChange={(v) =>
              update((s) => ({ ...s, postproc: { ...s.postproc, removeFillers: v } }))
            }
          />
        </Field>
        <Field
          label="Inserção"
          htmlFor="insert"
          hint="Automático digita textos curtos e cola os longos (devolvendo o que estava copiado)."
        >
          <Select
            id="insert"
            value={settings.insertMethod}
            onChange={(e) =>
              update((s) => ({ ...s, insertMethod: e.target.value as InsertMethod }))
            }
          >
            <option value="auto">Automático</option>
            <option value="type">Sempre digitar</option>
            <option value="paste">Sempre colar</option>
          </Select>
        </Field>
      </Section>

      <Section title="Atalho">
        <Field
          label="Atalho alternativo"
          htmlFor="alternate"
          hint={
            info?.hotkey.alternateError ? (
              <span className="text-recording">{info.hotkey.alternateError}</span>
            ) : (
              "Funciona junto com Ctrl + Win. Ex.: Ctrl+Alt+Space, Ctrl+Shift+D. Vazio desliga."
            )
          }
        >
          <CommitInput
            id="alternate"
            value={settings.hotkey.alternate}
            onCommit={(v) =>
              update((s) => ({ ...s, hotkey: { ...s.hotkey, alternate: v.trim() } }))
            }
          />
        </Field>
        <div className="grid grid-cols-2 gap-4">
          <Field label="Segurar a partir de (ms)" htmlFor="hold">
            <CommitInput
              id="hold"
              type="number"
              value={String(settings.hotkey.holdThresholdMs)}
              onCommit={(v) =>
                update((s) => ({ ...s, hotkey: { ...s.hotkey, holdThresholdMs: Number(v) } }))
              }
            />
          </Field>
          <Field label="Janela do toque duplo (ms)" htmlFor="double">
            <CommitInput
              id="double"
              type="number"
              value={String(settings.hotkey.doubleTapMs)}
              onCommit={(v) =>
                update((s) => ({ ...s, hotkey: { ...s.hotkey, doubleTapMs: Number(v) } }))
              }
            />
          </Field>
        </div>
      </Section>

      <Section title="Privacidade">
        <Field
          inline
          label="Autorizar processamento na nuvem"
          hint={
            settings.cloudConsentAt
              ? `Autorizado em ${new Date(settings.cloudConsentAt).toLocaleString("pt-BR")}. Desligar faz tudo rodar só no computador.`
              : "Sem autorização, o áudio e o texto não saem do computador (só o motor local funciona)."
          }
        >
          <Switch
            label="Autorizar processamento na nuvem"
            checked={settings.cloudConsentAt !== null}
            onCheckedChange={async (v) => {
              try {
                setSettings(await api.setCloudConsent(v));
              } catch (e) {
                setMessage(errorText(e));
              }
            }}
          />
        </Field>
        <Field label="Guardar histórico por" htmlFor="retention">
          <Select
            id="retention"
            value={String(settings.history.retentionDays)}
            onChange={(e) =>
              update((s) => ({ ...s, history: { retentionDays: Number(e.target.value) } }))
            }
          >
            <option value="0">Não guardar</option>
            <option value="7">7 dias</option>
            <option value="30">30 dias</option>
            <option value="90">90 dias</option>
            <option value="365">1 ano</option>
          </Select>
        </Field>
        <Field
          inline
          label="Guardar gravações (depuração)"
          hint={`Arquivos WAV em ${info?.dataDir ?? "%APPDATA%\\AnotAI"}\\recordings.`}
        >
          <Switch
            label="Guardar gravações"
            checked={settings.saveRecordings}
            onCheckedChange={(v) => update((s) => ({ ...s, saveRecordings: v }))}
          />
        </Field>
      </Section>

      <Section title="Sistema">
        <Field inline label="Iniciar com o Windows">
          <Switch
            label="Iniciar com o Windows"
            checked={settings.launchAtStartup}
            onCheckedChange={(v) => update((s) => ({ ...s, launchAtStartup: v }))}
          />
        </Field>
        <Field label="Tema" htmlFor="theme">
          <Select
            id="theme"
            value={settings.theme}
            onChange={(e) => update((s) => ({ ...s, theme: e.target.value as Theme }))}
          >
            <option value="system">Do sistema</option>
            <option value="light">Claro</option>
            <option value="dark">Escuro</option>
          </Select>
        </Field>
      </Section>
    </div>
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <Card>
      <h2 className="font-display font-semibold">{title}</h2>
      {hint && <p className="mt-0.5 text-muted-foreground text-xs">{hint}</p>}
      <div className="mt-4 flex flex-col gap-4">{children}</div>
    </Card>
  );
}

/** Campo de texto que só salva ao sair do campo ou apertar Enter. */
function CommitInput({
  id,
  value,
  onCommit,
  type = "text",
}: {
  id: string;
  value: string;
  onCommit: (v: string) => void;
  type?: string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft !== value) onCommit(draft);
  };
  return (
    <Input
      id={id}
      type={type}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
    />
  );
}

function KeyField({
  provider,
  label,
  saved,
  onSave,
}: {
  provider: KeyProvider;
  label: string;
  saved: boolean;
  onSave: (p: KeyProvider, v: string) => void;
}) {
  const [value, setValue] = useState("");
  const id = `key-${provider}`;
  return (
    <Field
      label={label}
      htmlFor={id}
      hint={
        <span className="flex items-center gap-1">
          {saved ? (
            <>
              <CheckCircle2 className="size-3.5 text-brand" /> Configurada
            </>
          ) : (
            <>
              <CircleDashed className="size-3.5" /> Não configurada
            </>
          )}
        </span>
      }
    >
      <div className="flex gap-2">
        <Input
          id={id}
          type="password"
          autoComplete="off"
          placeholder={saved ? "••••••••  (cole uma nova para trocar)" : "Cole a chave aqui"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <Button
          variant="outline"
          disabled={!value.trim()}
          onClick={() => {
            onSave(provider, value);
            setValue("");
          }}
        >
          Salvar
        </Button>
        {saved && (
          <Button variant="ghost" onClick={() => onSave(provider, "")}>
            Remover
          </Button>
        )}
      </div>
    </Field>
  );
}
