//! Transcrição de voz plugável (ADR-003).
//!
//! [`SttProvider`] é a interface; [`OpenAiCompatible`] atende OpenAI
//! (`gpt-4o-transcribe`) e Groq (`whisper-large-v3-turbo`), que usam o mesmo
//! endpoint `/audio/transcriptions`; [`Deepgram`] atende o Nova-3. O motor
//! local [`LocalWhisper`] (whisper.cpp) roda offline, atrás da mesma interface.
//!
//! Por enquanto a transcrição é feita ao soltar a tecla (áudio inteiro). O
//! streaming por WebSocket, que reduz a latência, vem numa próxima etapa.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anotai_audio::Recording;
use serde::{Deserialize, Serialize};

mod deepgram;
mod local;
mod openai;

pub use deepgram::Deepgram;
pub use local::{LocalWhisper, is_hallucination};
pub use openai::OpenAiCompatible;

/// Tempo máximo de uma transcrição.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Idioma falado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en")]
    En,
}

impl Language {
    /// Código ISO-639-1 (OpenAI/Groq).
    pub fn iso639_1(self) -> &'static str {
        match self {
            Self::PtBr => "pt",
            Self::En => "en",
        }
    }

    /// Código BCP-47 (Deepgram).
    pub fn bcp47(self) -> &'static str {
        match self {
            Self::PtBr => "pt-BR",
            Self::En => "en",
        }
    }
}

/// Provedores de nuvem disponíveis (`engine.cloudProvider`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudProvider {
    OpenAi,
    Groq,
    Deepgram,
}

impl CloudProvider {
    pub const ALL: [Self; 3] = [Self::OpenAi, Self::Groq, Self::Deepgram];

    /// Identificador usado no histórico (`engine`) e no cofre de chaves.
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Groq => "groq",
            Self::Deepgram => "deepgram",
        }
    }

    /// Cria o adaptador com a chave de API.
    pub fn client(self, api_key: String) -> Box<dyn SttProvider> {
        match self {
            Self::OpenAi => Box::new(OpenAiCompatible::openai(api_key)),
            Self::Groq => Box::new(OpenAiCompatible::groq(api_key)),
            Self::Deepgram => Box::new(Deepgram::new(api_key)),
        }
    }
}

/// Resultado de uma transcrição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    /// Ex.: "cloud:groq".
    pub engine: String,
}

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("chave de API ausente para {0}")]
    MissingKey(&'static str),
    #[error("chave de API recusada pelo provedor")]
    Unauthorized,
    #[error("limite de uso do provedor atingido; tente de novo em instantes")]
    RateLimited,
    #[error("o provedor respondeu {status}: {body}")]
    Http { status: u16, body: String },
    #[error("falha de rede: {0}")]
    Network(String),
    #[error("resposta inesperada do provedor: {0}")]
    Parse(String),
    #[error("falha ao preparar o áudio: {0}")]
    Audio(String),
    #[error("motor local: {0}")]
    Local(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Interface dos motores de transcrição.
pub trait SttProvider: Send + Sync {
    /// Identificador para o histórico, ex.: "cloud:openai".
    fn engine_id(&self) -> String;

    fn transcribe<'a>(
        &'a self,
        recording: &'a Recording,
        language: Language,
    ) -> BoxFuture<'a, Result<Transcript>>;
}

pub(crate) fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| Error::Network(e.to_string()))
}

pub(crate) fn wav_bytes(recording: &Recording) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(44 + recording.samples.len() * 2);
    recording
        .write_wav(&mut buf)
        .map_err(|e| Error::Audio(e.to_string()))?;
    Ok(buf)
}

/// Converte respostas não-2xx em erros legíveis.
pub(crate) async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    match status.as_u16() {
        401 | 403 => Err(Error::Unauthorized),
        429 => Err(Error::RateLimited),
        code => {
            let body = resp.text().await.unwrap_or_default();
            Err(Error::Http {
                status: code,
                body: body.chars().take(300).collect(),
            })
        }
    }
}

pub(crate) fn network(e: reqwest::Error) -> Error {
    Error::Network(e.to_string())
}
