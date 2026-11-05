import { Button, Card, Input } from "@anotai/ui";
import { Copy, Download, Search, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText, type Transcription } from "../lib/core";
import { useCoreStore } from "../lib/store";

const dateFmt = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "short" });

export function History() {
  const historyVersion = useCoreStore((s) => s.historyVersion);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<Transcription[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    void historyVersion;
    void reload;
    const id = setTimeout(() => {
      api
        .history(query, 200)
        .then(setItems)
        .catch((e) => setMessage(errorText(e)));
    }, 200);
    return () => clearTimeout(id);
  }, [query, historyVersion, reload]);

  const remove = async (id: string) => {
    try {
      await api.deleteHistoryItem(id);
      setItems((list) => list.filter((i) => i.id !== id));
    } catch (e) {
      setMessage(errorText(e));
    }
  };

  const copy = async (text: string) => {
    await navigator.clipboard.writeText(text);
    setMessage("Texto copiado.");
  };

  const exportAll = async () => {
    try {
      setMessage(`Exportado para ${await api.exportData()}`);
    } catch (e) {
      setMessage(errorText(e));
    }
  };

  const deleteAll = async () => {
    if (
      !window.confirm(
        "Apagar todo o histórico, o dicionário, as estatísticas e as chaves de API? Isso não pode ser desfeito.",
      )
    ) {
      return;
    }
    try {
      await api.deleteAllData();
      setMessage("Todos os dados foram apagados.");
      setReload((n) => n + 1);
    } catch (e) {
      setMessage(errorText(e));
    }
  };

  return (
    <div className="flex flex-col gap-5">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h1 className="font-display font-semibold text-2xl">Histórico</h1>
          <p className="text-muted-foreground text-sm">Tudo fica só neste computador.</p>
        </div>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" onClick={exportAll}>
            <Download /> Exportar
          </Button>
          <Button size="sm" variant="outline" onClick={deleteAll}>
            <Trash2 /> Apagar tudo
          </Button>
        </div>
      </header>

      <div className="relative">
        <Search className="absolute top-2.5 left-3 size-4 text-muted-foreground" />
        <Input
          aria-label="Buscar no histórico"
          placeholder="Buscar…"
          className="pl-9"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {message && (
        <p className="rounded-md bg-muted p-3 text-sm" role="status">
          {message}
        </p>
      )}

      {items.length === 0 ? (
        <p className="py-10 text-center text-muted-foreground text-sm">
          {query ? "Nada encontrado." : "Nenhum ditado ainda. Segure Ctrl + Win e fale."}
        </p>
      ) : (
        <ul className="flex flex-col gap-3">
          {items.map((item) => (
            <li key={item.id}>
              <Card className="p-4">
                <p className="whitespace-pre-wrap text-sm">{item.finalText}</p>
                <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-muted-foreground text-xs">
                  <span>{dateFmt.format(item.createdAt)}</span>
                  {item.targetApp && <span>{item.targetApp}</span>}
                  <span>{item.mode === "translation" ? "Tradução" : "Ditado"}</span>
                  <span>{(item.latencyMs / 1000).toFixed(1)} s</span>
                  {!item.inserted && <span className="text-recording">não inserido</span>}
                  <span className="ml-auto flex gap-1">
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label="Copiar texto"
                      onClick={() => copy(item.finalText)}
                    >
                      <Copy />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label="Apagar este ditado"
                      onClick={() => remove(item.id)}
                    >
                      <Trash2 />
                    </Button>
                  </span>
                </div>
              </Card>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
