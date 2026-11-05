import { Button, cn } from "@anotai/ui";
import { Check, Download, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  api,
  errorText,
  formatMb,
  type ModelProgress,
  type ModelStatus,
  onModelProgress,
} from "../lib/core";

/** Lista de modelos locais: escolher, baixar (com progresso) e remover. */
export function ModelsPanel({
  selected,
  onSelect,
  onChange,
}: {
  selected: string;
  onSelect: (id: string) => void;
  /** Avisado quando um modelo termina de baixar ou é removido. */
  onChange?: (models: ModelStatus[]) => void;
}) {
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const list = await api.modelsStatus();
    setModels(list);
    onChange?.(list);
  }, [onChange]);

  useEffect(() => {
    refresh().catch((e) => setError(errorText(e)));
    const unlisten = onModelProgress((p) => setProgress((all) => ({ ...all, [p.id]: p })));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  const download = async (id: string) => {
    setError(null);
    setModels((list) => list.map((m) => (m.id === id ? { ...m, downloading: true } : m)));
    try {
      await api.downloadModel(id);
      onSelect(id);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setProgress((all) => {
        const { [id]: _, ...rest } = all;
        return rest;
      });
      await refresh();
    }
  };

  const remove = async (id: string) => {
    try {
      await api.deleteModel(id);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="flex flex-col gap-2">
      {models.map((m) => {
        const p = progress[m.id];
        const pct = p ? Math.round((p.received / p.total) * 100) : 0;
        const isSelected = m.id === selected;
        return (
          <div
            key={m.id}
            className={cn(
              "flex items-center gap-3 rounded-md border p-3",
              isSelected ? "border-primary" : "border-border",
            )}
          >
            <button
              type="button"
              disabled={!m.installed}
              onClick={() => onSelect(m.id)}
              aria-pressed={isSelected}
              aria-label={`Usar o modelo ${m.label}`}
              className="flex min-w-0 flex-1 flex-col items-start text-left disabled:cursor-default"
            >
              <span className="flex items-center gap-1.5 font-medium text-sm">
                {isSelected && m.installed && <Check className="size-4 text-primary" />}
                {m.label} <span className="text-muted-foreground">· {formatMb(m.size)}</span>
              </span>
              <span className="text-muted-foreground text-xs">{m.description}</span>
              {m.downloading && (
                <span className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
                  <span
                    className="block h-full bg-primary transition-[width]"
                    style={{ width: `${pct}%` }}
                  />
                </span>
              )}
            </button>
            {m.installed ? (
              <Button
                size="icon"
                variant="ghost"
                aria-label={`Remover o modelo ${m.label}`}
                onClick={() => remove(m.id)}
              >
                <Trash2 />
              </Button>
            ) : (
              <Button
                size="sm"
                variant="outline"
                disabled={m.downloading}
                onClick={() => download(m.id)}
              >
                <Download /> {m.downloading ? `${pct}%` : "Baixar"}
              </Button>
            )}
          </div>
        );
      })}
      {error && <p className="text-recording text-sm">{error}</p>}
    </div>
  );
}
