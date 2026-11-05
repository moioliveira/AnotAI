import { Button, Card, Field, Input } from "@anotai/ui";
import { Plus, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { api, type DictionaryTerm, errorText } from "../lib/core";

export function Dictionary() {
  const [terms, setTerms] = useState<DictionaryTerm[]>([]);
  const [term, setTerm] = useState("");
  const [spokenAs, setSpokenAs] = useState("");
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    api
      .dictionary()
      .then(setTerms)
      .catch((e) => setMessage(errorText(e)));
  }, []);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    try {
      const created = await api.addTerm(term, spokenAs);
      setTerms((list) => [...list, created].sort((a, b) => a.term.localeCompare(b.term, "pt-BR")));
      setTerm("");
      setSpokenAs("");
      setMessage(null);
    } catch (err) {
      setMessage(errorText(err));
    }
  };

  const remove = async (id: string) => {
    try {
      await api.removeTerm(id);
      setTerms((list) => list.filter((t) => t.id !== id));
    } catch (err) {
      setMessage(errorText(err));
    }
  };

  return (
    <div className="flex flex-col gap-5">
      <header>
        <h1 className="font-display font-semibold text-2xl">Dicionário</h1>
        <p className="text-muted-foreground text-sm">
          Nomes, siglas e termos técnicos que devem sair sempre com a mesma grafia.
        </p>
      </header>

      <Card>
        <form onSubmit={add} className="grid grid-cols-[1fr_1fr_auto] items-end gap-3">
          <Field label="Termo" htmlFor="term">
            <Input
              id="term"
              placeholder="eSales"
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              required
            />
          </Field>
          <Field label="Como costuma ser ouvido (opcional)" htmlFor="spoken">
            <Input
              id="spoken"
              placeholder="i sales"
              value={spokenAs}
              onChange={(e) => setSpokenAs(e.target.value)}
            />
          </Field>
          <Button type="submit" disabled={!term.trim()}>
            <Plus /> Adicionar
          </Button>
        </form>
      </Card>

      {message && (
        <p className="rounded-md bg-muted p-3 text-sm" role="status">
          {message}
        </p>
      )}

      {terms.length === 0 ? (
        <p className="py-8 text-center text-muted-foreground text-sm">Nenhum termo ainda.</p>
      ) : (
        <ul className="divide-y divide-border rounded-lg border border-border bg-card">
          {terms.map((t) => (
            <li key={t.id} className="flex items-center gap-3 px-4 py-2.5 text-sm">
              <span className="font-medium">{t.term}</span>
              {t.spokenAs && <span className="text-muted-foreground">← “{t.spokenAs}”</span>}
              <Button
                size="icon"
                variant="ghost"
                className="ml-auto"
                aria-label={`Remover ${t.term}`}
                onClick={() => remove(t.id)}
              >
                <Trash2 />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
