-- Mantém o índice de busca (FTS5, external content) em sincronia com a tabela.

CREATE TRIGGER transcription_ai AFTER INSERT ON transcription BEGIN
  INSERT INTO transcription_fts(rowid, final_text) VALUES (new.rowid, new.final_text);
END;

CREATE TRIGGER transcription_ad AFTER DELETE ON transcription BEGIN
  INSERT INTO transcription_fts(transcription_fts, rowid, final_text)
  VALUES ('delete', old.rowid, old.final_text);
END;

CREATE TRIGGER transcription_au AFTER UPDATE OF final_text ON transcription BEGIN
  INSERT INTO transcription_fts(transcription_fts, rowid, final_text)
  VALUES ('delete', old.rowid, old.final_text);
  INSERT INTO transcription_fts(rowid, final_text) VALUES (new.rowid, new.final_text);
END;

CREATE INDEX transcription_created_at ON transcription(created_at DESC);
