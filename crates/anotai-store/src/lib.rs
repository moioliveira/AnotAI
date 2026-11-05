//! Histórico, dicionário e estatísticas em SQLite (Tech Spec 6.1), com busca
//! full-text (FTS5). Fica em `%APPDATA%\AnotAI\anotai.db`.

use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("erro no banco de dados: {0}")]
    Db(#[from] sqlx::Error),
    #[error("erro ao migrar o banco: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("termo já existe no dicionário: {0}")]
    DuplicateTerm(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Uma transcrição a gravar.
#[derive(Debug, Clone)]
pub struct NewTranscription {
    pub mode: String,
    pub trigger: String,
    pub raw_text: String,
    pub final_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub target_app: Option<String>,
    pub inserted: bool,
    pub duration_ms: i64,
    pub latency_ms: i64,
    pub engine: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    pub id: String,
    pub created_at: i64,
    pub mode: String,
    pub trigger: String,
    pub raw_text: String,
    pub final_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub target_app: Option<String>,
    pub inserted: bool,
    pub duration_ms: i64,
    pub latency_ms: i64,
    pub engine: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryTerm {
    pub id: String,
    pub term: String,
    pub spoken_as: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DailyStat {
    pub day: String,
    pub words: i64,
    pub sessions: i64,
    pub seconds: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Transforma o texto digitado numa consulta FTS5 segura: cada palavra vira
/// um termo entre aspas com prefixo (`"rel"*`), sem operadores do usuário.
pub fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

/// Colunas de `transcription` como literal para `concat!` (o sqlx 0.9 só
/// aceita SQL estático, como proteção contra injeção).
macro_rules! columns {
    () => {
        "id, created_at, mode, trigger, raw_text, final_text, source_lang, \
         target_lang, target_app, inserted, duration_ms, latency_ms, engine"
    };
}

impl Store {
    /// Abre (ou cria) o banco e aplica as migrações.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        Self::connect(options, 4).await
    }

    /// Banco temporário em memória (testes).
    pub async fn open_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        // Uma conexão só: cada conexão em memória seria um banco diferente.
        Self::connect(options, 1).await
    }

    async fn connect(options: SqliteConnectOptions, max: u32) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    /// Grava uma transcrição e atualiza as estatísticas do dia.
    pub async fn add_transcription(&self, t: &NewTranscription) -> Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        let created_at = now_ms();
        let words = t.final_text.split_whitespace().count() as i64;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO transcription (id, created_at, mode, trigger, raw_text, final_text, \
             source_lang, target_lang, target_app, inserted, duration_ms, latency_ms, engine) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(created_at)
        .bind(&t.mode)
        .bind(&t.trigger)
        .bind(&t.raw_text)
        .bind(&t.final_text)
        .bind(&t.source_lang)
        .bind(&t.target_lang)
        .bind(&t.target_app)
        .bind(t.inserted)
        .bind(t.duration_ms)
        .bind(t.latency_ms)
        .bind(&t.engine)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO daily_stat (day, words, sessions, seconds) \
             VALUES (date(? / 1000, 'unixepoch', 'localtime'), ?, 1, ?) \
             ON CONFLICT(day) DO UPDATE SET words = words + excluded.words, \
             sessions = sessions + 1, seconds = seconds + excluded.seconds",
        )
        .bind(created_at)
        .bind(words)
        .bind(t.duration_ms / 1000)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Mais recentes primeiro.
    pub async fn recent(&self, limit: i64) -> Result<Vec<Transcription>> {
        Ok(sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM transcription ORDER BY created_at DESC, id DESC LIMIT ?"
        ))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Busca full-text no texto final.
    pub async fn search(&self, input: &str, limit: i64) -> Result<Vec<Transcription>> {
        let Some(query) = fts_query(input) else {
            return self.recent(limit).await;
        };
        Ok(sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM transcription WHERE rowid IN \
             (SELECT rowid FROM transcription_fts WHERE transcription_fts MATCH ?) \
             ORDER BY created_at DESC, id DESC LIMIT ?"
        ))
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete_transcription(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM transcription WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Retenção (`history.retentionDays`). Devolve quantas linhas saíram.
    pub async fn purge_older_than(&self, days: u32) -> Result<u64> {
        let cutoff = now_ms() - i64::from(days) * 86_400_000;
        let r = sqlx::query("DELETE FROM transcription WHERE created_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }

    /// "Apagar tudo" (LGPD): histórico, estatísticas e dicionário.
    pub async fn delete_all(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for sql in [
            "DELETE FROM transcription",
            "DELETE FROM daily_stat",
            "DELETE FROM dictionary_term",
        ] {
            sqlx::query(sql).execute(&mut *tx).await?;
        }
        // Limpa também o índice FTS e compacta o arquivo.
        sqlx::query("INSERT INTO transcription_fts(transcription_fts) VALUES ('rebuild')")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }

    /// Exportação (LGPD): tudo em JSON.
    pub async fn export_json(&self) -> Result<serde_json::Value> {
        let history: Vec<Transcription> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM transcription ORDER BY created_at, id"
        ))
        .fetch_all(&self.pool)
        .await?;
        Ok(serde_json::json!({
            "exportedAt": now_ms(),
            "transcriptions": history,
            "dictionary": self.dictionary().await?,
            "dailyStats": self.stats(3650).await?,
        }))
    }

    pub async fn dictionary(&self) -> Result<Vec<DictionaryTerm>> {
        Ok(sqlx::query_as(
            "SELECT id, term, spoken_as, created_at FROM dictionary_term ORDER BY term COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn add_term(&self, term: &str, spoken_as: Option<&str>) -> Result<DictionaryTerm> {
        let row = DictionaryTerm {
            id: uuid::Uuid::now_v7().to_string(),
            term: term.trim().to_owned(),
            spoken_as: spoken_as
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            created_at: now_ms(),
        };
        let r = sqlx::query(
            "INSERT INTO dictionary_term (id, term, spoken_as, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&row.id)
        .bind(&row.term)
        .bind(&row.spoken_as)
        .bind(row.created_at)
        .execute(&self.pool)
        .await;
        match r {
            Ok(_) => Ok(row),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err(Error::DuplicateTerm(row.term))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn remove_term(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM dictionary_term WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Estatísticas dos últimos `days` dias (mais recente primeiro).
    pub async fn stats(&self, days: u32) -> Result<Vec<DailyStat>> {
        Ok(sqlx::query_as(
            "SELECT day, words, sessions, seconds FROM daily_stat \
             WHERE day >= date('now', 'localtime', ?) ORDER BY day DESC",
        )
        .bind(format!("-{days} days"))
        .fetch_all(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(text: &str) -> NewTranscription {
        NewTranscription {
            mode: "dictation".into(),
            trigger: "hold".into(),
            raw_text: text.to_lowercase(),
            final_text: text.into(),
            source_lang: "pt-BR".into(),
            target_lang: "pt-BR".into(),
            target_app: Some("NOTEPAD.EXE".into()),
            inserted: true,
            duration_ms: 3_000,
            latency_ms: 800,
            engine: "cloud:groq".into(),
        }
    }

    #[tokio::test]
    async fn insert_list_and_search() -> Result<()> {
        let store = Store::open_in_memory().await?;
        store
            .add_transcription(&sample("Relatório de vendas pronto."))
            .await?;
        store.add_transcription(&sample("Reunião às três.")).await?;

        let recent = store.recent(10).await?;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].final_text, "Reunião às três.");
        assert!(recent[0].inserted);

        let hits = store.search("relat", 10).await?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].final_text, "Relatório de vendas pronto.");
        // Operadores FTS digitados pelo usuário não quebram a consulta.
        assert!(store.search("AND OR \"(", 10).await.is_ok());
        Ok(())
    }

    /// Dois ditados no mesmo milissegundo (máquina rápida, modo contínuo)
    /// ainda saem na ordem em que foram gravados.
    #[tokio::test]
    async fn same_millisecond_keeps_insertion_order() -> Result<()> {
        let store = Store::open_in_memory().await?;
        for text in ["primeiro", "segundo", "terceiro"] {
            store.add_transcription(&sample(text)).await?;
        }
        sqlx::query("UPDATE transcription SET created_at = 1000")
            .execute(&store.pool)
            .await?;
        let order: Vec<String> = store
            .recent(10)
            .await?
            .into_iter()
            .map(|t| t.final_text)
            .collect();
        assert_eq!(order, ["terceiro", "segundo", "primeiro"]);
        let export = store.export_json().await?;
        assert_eq!(export["transcriptions"][0]["finalText"], "primeiro");
        Ok(())
    }

    #[tokio::test]
    async fn deleting_keeps_search_index_in_sync() -> Result<()> {
        let store = Store::open_in_memory().await?;
        let id = store.add_transcription(&sample("Texto apagável")).await?;
        store.delete_transcription(&id).await?;
        assert!(store.search("apagável", 10).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn daily_stats_accumulate() -> Result<()> {
        let store = Store::open_in_memory().await?;
        store.add_transcription(&sample("um dois três")).await?;
        store.add_transcription(&sample("quatro cinco")).await?;
        let stats = store.stats(1).await?;
        assert_eq!(stats.len(), 1);
        assert_eq!(
            (stats[0].words, stats[0].sessions, stats[0].seconds),
            (5, 2, 6)
        );
        Ok(())
    }

    #[tokio::test]
    async fn dictionary_crud_and_duplicates() -> Result<()> {
        let store = Store::open_in_memory().await?;
        let t = store.add_term(" eSales ", Some("i sales")).await?;
        assert_eq!(t.term, "eSales");
        assert!(matches!(
            store.add_term("eSales", None).await,
            Err(Error::DuplicateTerm(_))
        ));
        store.remove_term(&t.id).await?;
        assert!(store.dictionary().await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn delete_all_and_export() -> Result<()> {
        let store = Store::open_in_memory().await?;
        store.add_transcription(&sample("algo")).await?;
        store.add_term("AnotAI", None).await?;
        let export = store.export_json().await?;
        assert_eq!(export["transcriptions"].as_array().map(Vec::len), Some(1));
        store.delete_all().await?;
        assert!(store.recent(10).await?.is_empty());
        assert!(store.dictionary().await?.is_empty());
        assert!(store.search("algo", 10).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn retention_purges_only_old_rows() -> Result<()> {
        let store = Store::open_in_memory().await?;
        store.add_transcription(&sample("novo")).await?;
        sqlx::query("UPDATE transcription SET created_at = created_at - 40 * 86400000")
            .execute(&store.pool)
            .await?;
        store.add_transcription(&sample("recente")).await?;
        assert_eq!(store.purge_older_than(30).await?, 1);
        assert_eq!(store.recent(10).await?.len(), 1);
        Ok(())
    }

    #[test]
    fn fts_query_quotes_terms() {
        assert_eq!(
            fts_query("vendas maio").as_deref(),
            Some("\"vendas\"* \"maio\"*")
        );
        assert_eq!(fts_query("  "), None);
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"ab\"*"));
    }
}
