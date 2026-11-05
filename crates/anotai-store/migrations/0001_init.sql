-- Esquema inicial (Tech Spec, seção 6.1). Executado com sqlx migrate na inicialização.

CREATE TABLE transcription (
  id            TEXT PRIMARY KEY,          -- UUID v7
  created_at    INTEGER NOT NULL,          -- epoch ms
  mode          TEXT NOT NULL,             -- 'dictation' | 'translation'
  trigger       TEXT NOT NULL,             -- 'hold' | 'toggle'
  raw_text      TEXT NOT NULL,             -- saída do STT
  final_text    TEXT NOT NULL,             -- após correção/tradução
  source_lang   TEXT NOT NULL,             -- 'pt-BR' | 'en'
  target_lang   TEXT NOT NULL,
  target_app    TEXT,                      -- ex.: 'OUTLOOK.EXE'
  inserted      INTEGER NOT NULL,          -- 1 = inserido no cursor
  duration_ms   INTEGER NOT NULL,
  latency_ms    INTEGER NOT NULL,
  engine        TEXT NOT NULL              -- 'cloud:deepgram' | 'local:whisper'
);
CREATE VIRTUAL TABLE transcription_fts USING fts5(final_text, content='transcription');

CREATE TABLE dictionary_term (
  id          TEXT PRIMARY KEY,
  term        TEXT NOT NULL UNIQUE,        -- ex.: 'eSales'
  spoken_as   TEXT,                        -- ex.: 'i sales'
  created_at  INTEGER NOT NULL
);

CREATE TABLE daily_stat (
  day         TEXT PRIMARY KEY,            -- 'YYYY-MM-DD'
  words       INTEGER NOT NULL DEFAULT 0,
  sessions    INTEGER NOT NULL DEFAULT 0,
  seconds     INTEGER NOT NULL DEFAULT 0
);
