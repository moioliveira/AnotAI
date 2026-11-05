import { ptBR } from "./locales/pt-BR";

export type Messages = typeof ptBR;
export type Locale = "pt-BR";

const catalogs: Record<Locale, Messages> = { "pt-BR": ptBR };

/** Textos da UI. en e es entram depois do MVP. */
export function messages(locale: Locale = "pt-BR"): Messages {
  return catalogs[locale];
}
