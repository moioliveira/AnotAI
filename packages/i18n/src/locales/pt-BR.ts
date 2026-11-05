export const ptBR = {
  app: {
    name: "AnotAI",
    tagline: "Fale. O texto aparece no cursor, já corrigido.",
  },
  state: {
    idle: "Pronto",
    listening: "Ouvindo…",
    transcribing: "Transcrevendo…",
    inserting: "Inserindo…",
    error: "Algo deu errado",
  },
  mode: {
    dictation: "Ditado",
    translation: "Tradução EN→PT",
  },
  hint: {
    hold: "Segure Ctrl + Win para falar",
    doubleTap: "Toque duas vezes para o modo contínuo",
  },
  tray: {
    open: "Abrir AnotAI",
    quit: "Sair",
  },
} as const;
