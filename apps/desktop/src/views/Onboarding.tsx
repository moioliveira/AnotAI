import { Button, Card, cn, Field, Input, Select } from "@anotai/ui";
import { Cloud, Laptop, Mic } from "lucide-react";
import { useEffect, useState } from "react";
import { ModelsPanel } from "../components/ModelsPanel";
import {
  api,
  type CloudProvider,
  errorText,
  type KeyProvider,
  type ModelStatus,
  type Settings,
} from "../lib/core";
import { useCoreStore } from "../lib/store";

type Choice = "cloud" | "local";
const steps = ["Boas-vindas", "Privacidade", "Motor", "Microfone", "Pronto"] as const;

const providers: { id: CloudProvider; key: KeyProvider; name: string }[] = [
  { id: "groq", key: "groq", name: "Groq (Whisper large-v3-turbo)" },
  { id: "openAi", key: "openai", name: "OpenAI (gpt-4o-transcribe)" },
  { id: "deepgram", key: "deepgram", name: "Deepgram (Nova-3)" },
];

/** Primeira execução: consentimento LGPD, motor, microfone e atalho. */
export function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [choice, setChoice] = useState<Choice | null>(null);
  const [consent, setConsent] = useState(false);
  const [sttKey, setSttKey] = useState("");
  const [anthropicKey, setAnthropicKey] = useState("");
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch((e) => setError(errorText(e)));
  }, []);

  if (!settings) return null;

  const save = async (next: Settings) => {
    setSettings(await api.saveSettings(next));
  };

  const localReady = models.some((m) => m.id === settings.engine.localModel && m.installed);
  const provider = providers.find((p) => p.id === settings.engine.cloudProvider) ?? providers[0];

  const next = async () => {
    setError(null);
    try {
      if (step === 1 && choice) {
        const s = await api.setCloudConsent(choice === "cloud" && consent);
        await save({
          ...s,
          engine: { ...s.engine, mode: choice === "cloud" ? "auto" : "local" },
        });
      }
      if (step === 2 && choice === "cloud") {
        if (sttKey.trim() && provider) await api.setApiKey(provider.key, sttKey);
        if (anthropicKey.trim()) await api.setApiKey("anthropic", anthropicKey);
      }
      if (step === steps.length - 1) {
        await save({ ...settings, onboardingDone: true });
        onDone();
        return;
      }
      setStep((n) => n + 1);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const canAdvance =
    step === 1
      ? choice === "local" || (choice === "cloud" && consent)
      : step === 2
        ? choice === "local"
          ? localReady
          : sttKey.trim().length > 0
        : true;

  return (
    <div className="mx-auto flex min-h-screen max-w-xl flex-col justify-center gap-6 px-6 py-10">
      <ol className="flex gap-2" aria-label="Etapas">
        {steps.map((label, i) => (
          <li
            key={label}
            aria-current={i === step ? "step" : undefined}
            className={cn("h-1.5 flex-1 rounded-full", i <= step ? "bg-primary" : "bg-border")}
          >
            <span className="sr-only">{label}</span>
          </li>
        ))}
      </ol>

      {step === 0 && (
        <div className="flex flex-col gap-3">
          <img src="/anotai-simbolo.svg" alt="" className="size-14" />
          <h1 className="font-display font-semibold text-3xl">Bem-vindo ao AnotAI</h1>
          <p className="text-muted-foreground">
            Segure <strong>Ctrl + Win</strong> em qualquer app, fale e solte: o texto aparece no
            cursor, já pontuado e corrigido. Leva um minuto para configurar.
          </p>
        </div>
      )}

      {step === 1 && (
        <div className="flex flex-col gap-4">
          <h1 className="font-display font-semibold text-2xl">Onde sua voz é processada?</h1>
          <ChoiceCard
            icon={<Cloud />}
            title="Na nuvem (melhor qualidade)"
            selected={choice === "cloud"}
            onClick={() => setChoice("cloud")}
          >
            O áudio de cada ditado vai para o provedor de transcrição escolhido, e o texto vai para
            a Anthropic (Claude) para correção e tradução. O AnotAI não guarda áudio; o tratamento
            pelos provedores segue a política de cada um. Se a rede cair, o modelo local (se
            baixado) assume.
          </ChoiceCard>
          <ChoiceCard
            icon={<Laptop />}
            title="Só neste computador (privado)"
            selected={choice === "local"}
            onClick={() => setChoice("local")}
          >
            Transcrição offline com o Whisper. Nada sai da máquina; a qualidade e a velocidade
            dependem do processador, e não há correção pela norma culta nem tradução.
          </ChoiceCard>
          {choice === "cloud" && (
            <label className="flex items-start gap-2 text-sm">
              <input
                type="checkbox"
                className="mt-0.5 size-4 accent-[var(--color-primary)]"
                checked={consent}
                onChange={(e) => setConsent(e.target.checked)}
              />
              Autorizo o envio do meu áudio e do texto ditado a esses provedores para transcrição,
              correção e tradução. Posso revogar quando quiser em Configurações.
            </label>
          )}
        </div>
      )}

      {step === 2 && choice === "cloud" && (
        <div className="flex flex-col gap-4">
          <h1 className="font-display font-semibold text-2xl">Chaves de API</h1>
          <p className="text-muted-foreground text-sm">
            Ficam no Gerenciador de Credenciais do Windows e nunca são exibidas de volta.
          </p>
          <Field label="Provedor de transcrição" htmlFor="ob-provider">
            <Select
              id="ob-provider"
              value={settings.engine.cloudProvider}
              onChange={(e) =>
                save({
                  ...settings,
                  engine: { ...settings.engine, cloudProvider: e.target.value as CloudProvider },
                })
              }
            >
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label={`Chave de ${provider?.name.split(" ")[0]}`} htmlFor="ob-stt">
            <Input
              id="ob-stt"
              type="password"
              value={sttKey}
              onChange={(e) => setSttKey(e.target.value)}
            />
          </Field>
          <Field
            label="Chave da Anthropic (opcional)"
            htmlFor="ob-anthropic"
            hint="Para corrigir pela norma culta e traduzir. Sem ela, o texto sai só com as regras básicas."
          >
            <Input
              id="ob-anthropic"
              type="password"
              value={anthropicKey}
              onChange={(e) => setAnthropicKey(e.target.value)}
            />
          </Field>
        </div>
      )}

      {step === 2 && choice === "local" && (
        <div className="flex flex-col gap-4">
          <h1 className="font-display font-semibold text-2xl">Baixe um modelo</h1>
          <p className="text-muted-foreground text-sm">
            Vem do Hugging Face e é conferido (SHA-256) antes de usar.
          </p>
          <ModelsPanel
            selected={settings.engine.localModel}
            onSelect={(id) => save({ ...settings, engine: { ...settings.engine, localModel: id } })}
            onChange={setModels}
          />
        </div>
      )}

      {step === 3 && <MicTest />}

      {step === 4 && (
        <div className="flex flex-col gap-3">
          <h1 className="font-display font-semibold text-2xl">Pronto!</h1>
          <ul className="flex list-disc flex-col gap-2 pl-5 text-sm">
            <li>
              <strong>Segure Ctrl + Win</strong>, fale e solte: o texto entra onde está o cursor.
            </li>
            <li>
              <strong>Toque duas vezes</strong> para o modo contínuo: cada frase entra quando você
              faz uma pausa. Um toque encerra.
            </li>
            <li>Diga “vírgula”, “ponto final”, “nova linha” ou “novo parágrafo” para pontuar.</li>
            <li>O AnotAI fica na bandeja; o menu dele alterna para a tradução EN→PT.</li>
          </ul>
        </div>
      )}

      {error && <p className="rounded-md bg-recording/10 p-3 text-sm">{error}</p>}

      <div className="flex justify-between">
        <Button variant="ghost" disabled={step === 0} onClick={() => setStep((n) => n - 1)}>
          Voltar
        </Button>
        <Button disabled={!canAdvance} onClick={next}>
          {step === steps.length - 1 ? "Começar a usar" : "Continuar"}
        </Button>
      </div>
    </div>
  );
}

function ChoiceCard({
  icon,
  title,
  selected,
  onClick,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  selected: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={selected}
      className={cn(
        "flex gap-3 rounded-lg border p-4 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&_svg]:mt-0.5 [&_svg]:size-5 [&_svg]:shrink-0",
        selected ? "border-primary bg-primary/5" : "border-border hover:bg-muted",
      )}
    >
      {icon}
      <span className="flex flex-col gap-1">
        <span className="font-medium">{title}</span>
        <span className="text-muted-foreground text-sm">{children}</span>
      </span>
    </button>
  );
}

/** Liga o microfone enquanto a etapa está aberta e mostra o nível. */
function MicTest() {
  const level = useCoreStore((s) => s.level);
  const [peak, setPeak] = useState(0);

  useEffect(() => {
    api.micTest(true).catch(console.error);
    return () => {
      api.micTest(false).catch(console.error);
    };
  }, []);

  useEffect(() => setPeak((p) => Math.max(p, level)), [level]);

  const pct = Math.min(100, Math.round(level * 400));
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display font-semibold text-2xl">Teste o microfone</h1>
      <p className="text-muted-foreground text-sm">
        Fale algo: a barra deve se mexer. Nada é gravado.
      </p>
      <Card className="flex items-center gap-3">
        <Mic className="size-5 text-muted-foreground" />
        <meter className="sr-only" aria-label="Nível do microfone" min={0} max={100} value={pct} />
        <div aria-hidden className="h-3 flex-1 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full bg-recording transition-[width] duration-75"
            style={{ width: `${pct}%` }}
          />
        </div>
      </Card>
      {peak > 0.02 ? (
        <p className="text-sm">Ouvindo bem. ✓</p>
      ) : (
        <p className="text-muted-foreground text-sm">
          Se a barra não se mexer, confira o microfone padrão nas configurações de som do Windows.
        </p>
      )}
    </div>
  );
}
