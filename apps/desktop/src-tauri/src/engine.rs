//! Escolha do motor de transcrição (`engine.mode`) e recursos carregados na
//! memória: o modelo local (whisper.cpp) e o VAD.
//!
//! - **Nuvem**: precisa de consentimento (LGPD) e da chave do provedor.
//! - **Local**: precisa do modelo baixado; o áudio não sai do computador.
//! - **Auto**: nuvem; se a rede ou o provedor falhar (ou não houver
//!   consentimento/chave), o modelo local, se estiver baixado.

use std::sync::{Arc, Mutex};

use anotai_audio::Recording;
use anotai_stt::{CloudProvider, Language, LocalWhisper, SttProvider, Transcript};
use anotai_vad::SileroVad;
use tauri::{AppHandle, Manager};

use crate::settings::{EngineMode, Settings};
use crate::{models, secrets};

/// Modelo local carregado (reaproveitado entre ditados).
#[derive(Default)]
pub struct LocalEngine(Mutex<Option<Arc<LocalWhisper>>>);

/// VAD carregado (modo contínuo). `None` se o recurso não existir.
pub struct Vad(pub Option<Arc<Mutex<SileroVad>>>);

pub fn init(app: &AppHandle) {
    app.manage(LocalEngine::default());
    let vad = models::vad_path(app).and_then(|p| match SileroVad::load(&p) {
        Ok(v) => Some(Arc::new(Mutex::new(v))),
        Err(e) => {
            log::warn!("VAD indisponível: {e}");
            None
        }
    });
    if vad.is_none() {
        log::warn!("modo contínuo sem VAD: o texto só sai ao encerrar");
    }
    app.manage(Vad(vad));
}

pub fn vad(app: &AppHandle) -> Option<Arc<Mutex<SileroVad>>> {
    app.try_state::<Vad>().and_then(|v| v.0.clone())
}

pub fn unload_local(app: &AppHandle) {
    if let Some(engine) = app.try_state::<LocalEngine>()
        && let Ok(mut slot) = engine.0.lock()
    {
        slot.take();
    }
}

/// Mensagem para o usuário quando nenhum motor serve.
#[derive(Debug)]
pub struct EngineError {
    pub title: &'static str,
    pub body: String,
    /// Abrir a tela de configurações ajuda a resolver.
    pub needs_setup: bool,
}

impl EngineError {
    fn setup(title: &'static str, body: impl Into<String>) -> Self {
        Self {
            title,
            body: body.into(),
            needs_setup: true,
        }
    }

    fn failure(err: &anotai_stt::Error) -> Self {
        Self {
            title: "Falha na transcrição",
            body: err.to_string(),
            needs_setup: matches!(
                err,
                anotai_stt::Error::Unauthorized | anotai_stt::Error::MissingKey(_)
            ),
        }
    }
}

fn provider_name(p: CloudProvider) -> &'static str {
    match p {
        CloudProvider::OpenAi => "OpenAI",
        CloudProvider::Groq => "Groq",
        CloudProvider::Deepgram => "Deepgram",
    }
}

fn cloud(cfg: &Settings) -> Result<Box<dyn SttProvider>, EngineError> {
    if cfg.cloud_consent_at.is_none() {
        return Err(EngineError::setup(
            "Nuvem não autorizada",
            "Autorize o processamento na nuvem em Configurações ou use o motor local.",
        ));
    }
    let provider = cfg.engine.cloud_provider;
    secrets::get(provider.id())
        .map(|k| provider.client(k))
        .ok_or_else(|| {
            EngineError::setup(
                "Configure a transcrição",
                format!(
                    "Informe a chave de API de {} em Configurações.",
                    provider_name(provider)
                ),
            )
        })
}

/// Carrega (ou reaproveita) o modelo local escolhido.
async fn local(
    app: &AppHandle,
    cfg: &Settings,
    vocabulary: &[String],
) -> Result<Arc<LocalWhisper>, EngineError> {
    let model = models::find(&cfg.engine.local_model)
        .filter(|m| models::installed(m))
        .ok_or_else(|| {
            EngineError::setup(
                "Modelo local não instalado",
                "Baixe um modelo em Configurações → Transcrição.",
            )
        })?;
    let state = app.state::<LocalEngine>();
    let cached = state
        .0
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .filter(|w| w.model_id() == model.id);
    let whisper = match cached {
        Some(w) => w,
        None => {
            let path = models::path(model).ok_or_else(|| {
                EngineError::setup("Modelo local não instalado", "APPDATA não definido")
            })?;
            let vad = models::vad_path(app);
            let id = model.id;
            let loaded =
                tauri::async_runtime::spawn_blocking(move || LocalWhisper::load(&path, id, vad))
                    .await
                    .map_err(|e| EngineError::failure(&anotai_stt::Error::Local(e.to_string())))?
                    .map_err(|e| EngineError::failure(&e))?;
            let loaded = Arc::new(loaded);
            if let Ok(mut slot) = state.0.lock() {
                *slot = Some(Arc::clone(&loaded));
            }
            log::info!("modelo local {} carregado", model.id);
            loaded
        }
    };
    whisper.set_vocabulary(vocabulary);
    Ok(whisper)
}

/// Erros em que vale tentar o motor local.
fn should_fall_back(err: &anotai_stt::Error) -> bool {
    match err {
        anotai_stt::Error::Network(_) | anotai_stt::Error::RateLimited => true,
        anotai_stt::Error::Http { status, .. } => *status >= 500,
        _ => false,
    }
}

fn has_local_model(cfg: &Settings) -> bool {
    models::find(&cfg.engine.local_model).is_some_and(models::installed)
}

/// Transcreve com o motor configurado.
pub async fn transcribe(
    app: &AppHandle,
    cfg: &Settings,
    recording: &Recording,
    language: Language,
    vocabulary: &[String],
) -> Result<Transcript, EngineError> {
    match cfg.engine.mode {
        EngineMode::Cloud => cloud(cfg)?
            .transcribe(recording, language)
            .await
            .map_err(|e| EngineError::failure(&e)),
        EngineMode::Local => local(app, cfg, vocabulary)
            .await?
            .transcribe(recording, language)
            .await
            .map_err(|e| EngineError::failure(&e)),
        EngineMode::Auto => match cloud(cfg) {
            Ok(provider) => match provider.transcribe(recording, language).await {
                Ok(t) => Ok(t),
                Err(e) if should_fall_back(&e) && has_local_model(cfg) => {
                    log::warn!("nuvem falhou ({e}); usando o motor local");
                    local(app, cfg, vocabulary)
                        .await?
                        .transcribe(recording, language)
                        .await
                        .map_err(|e| EngineError::failure(&e))
                }
                Err(e) => Err(EngineError::failure(&e)),
            },
            Err(setup) if has_local_model(cfg) => {
                log::info!("{}: usando o motor local", setup.title);
                local(app, cfg, vocabulary)
                    .await?
                    .transcribe(recording, language)
                    .await
                    .map_err(|e| EngineError::failure(&e))
            }
            Err(setup) => Err(setup),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_errors_fall_back() {
        assert!(should_fall_back(&anotai_stt::Error::Network("x".into())));
        assert!(should_fall_back(&anotai_stt::Error::RateLimited));
        assert!(should_fall_back(&anotai_stt::Error::Http {
            status: 503,
            body: String::new()
        }));
        assert!(!should_fall_back(&anotai_stt::Error::Http {
            status: 400,
            body: String::new()
        }));
        assert!(!should_fall_back(&anotai_stt::Error::Unauthorized));
    }

    #[test]
    fn cloud_requires_consent() {
        let cfg = Settings::default();
        assert!(cfg.cloud_consent_at.is_none());
        assert!(matches!(cloud(&cfg), Err(e) if e.title == "Nuvem não autorizada"));
    }
}
