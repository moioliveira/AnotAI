//! Pós-processamento em cadeia (Tech Spec 5.2): comandos de voz → hesitações
//! → dicionário → LLM (correção, ou tradução + correção, numa chamada só).
//! Se o LLM falhar ou passar do tempo, o texto das regras já sai pontuado.

mod llm;
mod rules;

use std::time::Duration;

pub use llm::{API_VERSION, DEFAULT_MODEL, LlmConfig, LlmError, Task, rewrite};
pub use rules::{Term, apply_dictionary, apply_voice_commands, remove_fillers, tidy};

/// Tempo-base do LLM (Tech Spec: 1,5 s), mais uma folga para textos longos.
pub const BASE_TIMEOUT: Duration = Duration::from_millis(1_500);

/// Opções vindas das configurações (`postproc.*`, `mode.current`).
#[derive(Debug, Clone)]
pub struct Options {
    pub task: Task,
    pub remove_fillers: bool,
    pub correction: bool,
    pub dictionary: Vec<Term>,
    /// `None` = sem chave da Anthropic: só as regras.
    pub llm: Option<LlmConfig>,
}

/// O que aconteceu com a etapa do LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmOutcome {
    Applied,
    Skipped,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processed {
    pub text: String,
    pub llm: LlmOutcome,
}

/// Limite de tempo para um texto deste tamanho.
pub fn timeout_for(text: &str) -> Duration {
    BASE_TIMEOUT + Duration::from_millis(text.chars().count() as u64 * 2)
}

/// Só as regras determinísticas (offline, instantâneo).
pub fn apply_rules(raw: &str, opts: &Options) -> String {
    let mut text = raw.to_owned();
    // Comandos falados e hesitações são de português; no modo tradução o
    // texto falado é inglês e fica para o LLM.
    if opts.task == Task::Correct {
        text = apply_voice_commands(&text);
        if opts.remove_fillers {
            text = remove_fillers(&text);
        }
    }
    text = apply_dictionary(&text, &opts.dictionary);
    tidy(&text)
}

/// Cadeia completa.
pub async fn process(raw: &str, opts: &Options) -> Processed {
    let ruled = apply_rules(raw, opts);
    let wants_llm = opts.correction || opts.task == Task::Translate;
    let Some(config) = opts.llm.as_ref().filter(|_| wants_llm && !ruled.is_empty()) else {
        return Processed {
            text: ruled,
            llm: LlmOutcome::Skipped,
        };
    };
    match rewrite(
        config,
        opts.task,
        &ruled,
        &opts.dictionary,
        timeout_for(&ruled),
    )
    .await
    {
        Ok(text) => Processed {
            // O dicionário de novo: garante a grafia mesmo se o modelo errar.
            text: apply_dictionary(&text, &opts.dictionary),
            llm: LlmOutcome::Applied,
        },
        Err(err) => {
            tracing::warn!("LLM indisponível, usando o texto das regras: {err}");
            Processed {
                text: ruled,
                llm: LlmOutcome::Failed(err.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(llm: Option<LlmConfig>) -> Options {
        Options {
            task: Task::Correct,
            remove_fillers: true,
            correction: true,
            dictionary: vec![],
            llm,
        }
    }

    #[tokio::test]
    async fn without_key_uses_rules_only() {
        let p = process("olá vírgula tudo bem", &opts(None)).await;
        assert_eq!(p.text, "Olá, tudo bem");
        assert_eq!(p.llm, LlmOutcome::Skipped);
    }

    #[tokio::test]
    async fn llm_failure_falls_back_to_ruled_text() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/v1/messages")
            .with_status(500)
            .create_async()
            .await;
        let mut cfg = LlmConfig::new("k".into());
        cfg.base_url = server.url();
        let p = process("oi ponto final", &opts(Some(cfg))).await;
        assert_eq!(p.text, "Oi.");
        assert!(matches!(p.llm, LlmOutcome::Failed(_)));
    }

    #[tokio::test]
    async fn slow_llm_times_out_and_falls_back() {
        // Porta sem servidor que aceite e responda: simula rede travada
        // apontando para um endereço não roteável.
        let mut cfg = LlmConfig::new("k".into());
        cfg.base_url = "http://10.255.255.1:9".into();
        let started = std::time::Instant::now();
        let p = process("teste", &opts(Some(cfg))).await;
        assert_eq!(p.text, "Teste");
        assert!(matches!(p.llm, LlmOutcome::Failed(_)));
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn timeout_grows_with_text() {
        assert_eq!(timeout_for(""), BASE_TIMEOUT);
        assert!(timeout_for(&"a".repeat(1000)) > Duration::from_secs(3));
    }
}
