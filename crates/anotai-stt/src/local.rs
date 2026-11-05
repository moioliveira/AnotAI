//! Motor local: whisper.cpp (via whisper-rs), offline e privado. Roda na CPU;
//! aceleração por Vulkan/CUDA entra depois por feature.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anotai_audio::Recording;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{BoxFuture, Error, Language, Result, SttProvider, Transcript};

/// Frases que o Whisper "alucina" em trechos sem fala (vêm de legendas do
/// treino). Se a transcrição inteira for só isso, é descartada.
const HALLUCINATIONS: &[&str] = &[
    "legendas pela comunidade amara.org",
    "obrigado por assistir",
    "obrigada por assistir",
    "inscreva-se no canal",
    "legenda adriana zanotto",
    "thank you for watching",
    "thanks for watching",
    "subtitles by the amara.org community",
    "[música]",
    "(música)",
    "[music]",
];

pub fn is_hallucination(text: &str) -> bool {
    let normalized = text
        .trim()
        .trim_matches(|c: char| {
            c.is_ascii_punctuation() && c != '[' && c != ']' && c != '(' && c != ')'
        })
        .trim()
        .to_lowercase();
    normalized.is_empty() || HALLUCINATIONS.contains(&normalized.as_str())
}

/// Modelo carregado na memória. Carregar leva alguns segundos: crie uma vez
/// e reaproveite.
pub struct LocalWhisper {
    model_id: String,
    ctx: Arc<WhisperContext>,
    vad_model: Option<PathBuf>,
    /// Termos do dicionário, usados como contexto para o modelo.
    prompt: Mutex<String>,
}

impl LocalWhisper {
    /// Carrega o modelo (bloqueia; chame fora da thread de UI).
    pub fn load(model_path: &Path, model_id: &str, vad_model: Option<PathBuf>) -> Result<Self> {
        // Logs do whisper.cpp vão para o `tracing` (silenciosos sem assinante)
        // em vez de poluir o stderr.
        whisper_rs::install_logging_hooks();
        let path = model_path
            .to_str()
            .ok_or_else(|| Error::Local("caminho do modelo inválido".into()))?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu(false);
        let ctx = WhisperContext::new_with_params(path, params)
            .map_err(|e| Error::Local(format!("falha ao carregar o modelo: {e}")))?;
        Ok(Self {
            model_id: model_id.to_owned(),
            ctx: Arc::new(ctx),
            vad_model: vad_model.filter(|p| p.exists()),
            prompt: Mutex::new(String::new()),
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Termos que o modelo deve reconhecer (dicionário pessoal).
    pub fn set_vocabulary(&self, terms: &[String]) {
        if let Ok(mut p) = self.prompt.lock() {
            *p = terms.join(", ");
        }
    }

    fn run_blocking(
        ctx: &WhisperContext,
        samples: &[f32],
        language: Language,
        prompt: &str,
        vad_model: Option<&Path>,
    ) -> Result<String> {
        let mut state = ctx
            .create_state()
            .map_err(|e| Error::Local(e.to_string()))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(8))
            .unwrap_or(4) as i32;
        params.set_n_threads(threads);
        params.set_language(Some(language.iso639_1()));
        params.set_translate(false);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }
        // O VAD pula trechos de silêncio: mais rápido e menos alucinação.
        let vad_path = vad_model.and_then(Path::to_str);
        if let Some(vad) = vad_path {
            params.set_vad_model_path(Some(vad));
            params.enable_vad(true);
        }
        state
            .full(params, samples)
            .map_err(|e| Error::Local(e.to_string()))?;

        let mut text = String::new();
        for i in 0..state.full_n_segments() {
            if let Some(segment) = state.get_segment(i)
                && let Ok(s) = segment.to_str_lossy()
            {
                text.push_str(&s);
            }
        }
        let text = text.trim().to_owned();
        Ok(if is_hallucination(&text) {
            String::new()
        } else {
            text
        })
    }
}

impl SttProvider for LocalWhisper {
    fn engine_id(&self) -> String {
        format!("local:whisper-{}", self.model_id)
    }

    fn transcribe<'a>(
        &'a self,
        recording: &'a Recording,
        language: Language,
    ) -> BoxFuture<'a, Result<Transcript>> {
        let ctx = Arc::clone(&self.ctx);
        let samples = recording.samples.clone();
        let prompt = self.prompt.lock().map(|p| p.clone()).unwrap_or_default();
        let vad = self.vad_model.clone();
        let engine = self.engine_id();
        Box::pin(async move {
            // CPU pesada: fora do executor assíncrono.
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let _ = tx.send(Self::run_blocking(
                    &ctx,
                    &samples,
                    language,
                    &prompt,
                    vad.as_deref(),
                ));
            });
            let text = rx
                .await
                .map_err(|_| Error::Local("a thread do whisper parou".into()))??;
            Ok(Transcript { text, engine })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_subtitle_hallucinations_only_when_alone() {
        assert!(is_hallucination(" Legendas pela comunidade Amara.org "));
        assert!(is_hallucination("Obrigado por assistir!"));
        assert!(is_hallucination("[Música]"));
        assert!(is_hallucination("   "));
        assert!(!is_hallucination(
            "Obrigado por assistir à reunião de ontem."
        ));
        assert!(!is_hallucination("Bom dia"));
    }
}
