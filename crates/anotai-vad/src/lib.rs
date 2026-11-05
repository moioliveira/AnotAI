//! Detecção de voz e segmentação do modo contínuo (Tech Spec 5.1).
//!
//! [`Segmenter`] acumula o áudio que chega do microfone e, a cada pausa
//! longa o bastante, devolve o trecho falado para ser transcrito e inserido,
//! sem esperar o usuário encerrar o modo contínuo. A detecção de fala em si
//! fica atrás de [`SpeechDetector`]; [`SileroVad`] é a implementação real
//! (modelo Silero rodando no whisper.cpp).

use std::path::Path;

use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

/// Taxa do áudio esperado (a mesma do STT).
pub const SAMPLE_RATE: usize = 16_000;

const fn ms(n: usize) -> usize {
    n * SAMPLE_RATE / 1000
}

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("falha ao carregar o modelo de VAD: {0}")]
    Load(String),
    #[error("falha na detecção de voz: {0}")]
    Detect(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Trechos com fala, em índices de amostra (início, fim).
pub trait SpeechDetector {
    fn speech_segments(&mut self, samples: &[f32]) -> Result<Vec<(usize, usize)>>;
}

/// Silero VAD (via whisper.cpp).
pub struct SileroVad {
    ctx: WhisperVadContext,
}

impl SileroVad {
    pub fn load(model: &Path) -> Result<Self> {
        let path = model
            .to_str()
            .ok_or_else(|| Error::Load("caminho inválido".into()))?;
        let mut params = WhisperVadContextParams::new();
        params.set_n_threads(2);
        params.set_use_gpu(false);
        let ctx = WhisperVadContext::new(path, params).map_err(|e| Error::Load(e.to_string()))?;
        Ok(Self { ctx })
    }
}

impl SpeechDetector for SileroVad {
    fn speech_segments(&mut self, samples: &[f32]) -> Result<Vec<(usize, usize)>> {
        let mut params = WhisperVadParams::new();
        params.set_threshold(0.5);
        params.set_min_speech_duration(200);
        params.set_min_silence_duration(300);
        params.set_speech_pad(30);
        let segments = self
            .ctx
            .segments_from_samples(params, samples)
            .map_err(|e| Error::Detect(e.to_string()))?;
        // O whisper.cpp devolve centésimos de segundo.
        let to_index =
            |cs: f32| ((cs.max(0.0) * SAMPLE_RATE as f32 / 100.0) as usize).min(samples.len());
        Ok(segments
            .map(|s| (to_index(s.start), to_index(s.end)))
            .filter(|(a, b)| b > a)
            .collect())
    }
}

/// Parâmetros do corte (em amostras).
#[derive(Debug, Clone, Copy)]
pub struct SegmenterConfig {
    /// Pausa que encerra uma frase.
    pub min_silence: usize,
    /// Fala total mínima para valer a pena transcrever.
    pub min_speech: usize,
    /// Corta mesmo sem pausa a partir deste tamanho (fala corrida).
    pub max_chunk: usize,
    /// Folga mantida depois do fim da fala.
    pub pad: usize,
    /// Silêncio acima disto é descartado sem transcrever.
    pub max_leading_silence: usize,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            // Pausas menores acontecem no meio da frase ("eu acho que…");
            // cortar ali gera fragmentos mal pontuados.
            min_silence: ms(1_200),
            min_speech: ms(250),
            max_chunk: ms(25_000),
            pad: ms(200),
            max_leading_silence: ms(3_000),
        }
    }
}

/// Acumula áudio e libera frases completas.
#[derive(Debug, Default)]
pub struct Segmenter {
    pending: Vec<f32>,
    config: SegmenterConfig,
}

impl Segmenter {
    pub fn new(config: SegmenterConfig) -> Self {
        Self {
            pending: Vec::new(),
            config,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.pending.extend_from_slice(samples);
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Devolve uma frase pronta, se houver.
    pub fn poll(&mut self, detector: &mut dyn SpeechDetector) -> Result<Option<Vec<f32>>> {
        let c = self.config;
        if self.pending.len() < c.min_silence {
            return Ok(None);
        }
        let segments = detector.speech_segments(&self.pending)?;
        let Some(&(_, last_end)) = segments.last() else {
            // Só silêncio: descarta, mantendo um pouco para não cortar o
            // início da próxima palavra.
            if self.pending.len() > c.max_leading_silence {
                let cut = self.pending.len() - c.pad;
                self.pending.drain(..cut);
            }
            return Ok(None);
        };
        let trailing_silence = self.pending.len() - last_end;
        let too_long = self.pending.len() >= c.max_chunk;
        if trailing_silence < c.min_silence && !too_long {
            return Ok(None);
        }
        let speech: usize = segments.iter().map(|(a, b)| b - a).sum();
        let start = segments[0].0.saturating_sub(c.pad);
        let cut = (last_end + c.pad).min(self.pending.len());
        let chunk: Vec<f32> = self.pending.drain(..cut).skip(start).collect();
        Ok((speech >= c.min_speech).then_some(chunk))
    }

    /// O que sobrou ao encerrar (devolvido se tiver fala).
    pub fn finish(self, detector: &mut dyn SpeechDetector) -> Result<Option<Vec<f32>>> {
        if self.pending.len() < self.config.min_speech {
            return Ok(None);
        }
        let speech: usize = detector
            .speech_segments(&self.pending)?
            .iter()
            .map(|(a, b)| b - a)
            .sum();
        Ok((speech >= self.config.min_speech).then_some(self.pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Detector de mentira: fala = amostras com |x| > 0,1.
    struct Threshold;

    impl SpeechDetector for Threshold {
        fn speech_segments(&mut self, s: &[f32]) -> Result<Vec<(usize, usize)>> {
            let mut out = Vec::new();
            let mut start = None;
            for (i, x) in s.iter().enumerate() {
                match (x.abs() > 0.1, start) {
                    (true, None) => start = Some(i),
                    (false, Some(a)) => {
                        out.push((a, i));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(a) = start {
                out.push((a, s.len()));
            }
            Ok(out)
        }
    }

    fn speech(len_ms: usize) -> Vec<f32> {
        vec![0.5; ms(len_ms)]
    }

    fn silence(len_ms: usize) -> Vec<f32> {
        vec![0.0; ms(len_ms)]
    }

    #[test]
    fn waits_for_pause_then_releases_phrase() -> Result<()> {
        let mut seg = Segmenter::default();
        seg.push(&speech(1_000));
        seg.push(&silence(800));
        assert!(seg.poll(&mut Threshold)?.is_none(), "pausa curta não corta");
        seg.push(&silence(500));
        let chunk = seg
            .poll(&mut Threshold)?
            .ok_or(Error::Detect("sem frase".into()))?;
        assert_eq!(chunk.len(), ms(1_000) + ms(200));
        // O silêncio restante fica pendente.
        assert_eq!(seg.pending_len(), ms(1_100));
        Ok(())
    }

    #[test]
    fn two_phrases_come_out_in_order() -> Result<()> {
        let mut seg = Segmenter::default();
        let mut out = Vec::new();
        for part in [speech(800), silence(1_300), speech(400), silence(1_300)] {
            seg.push(&part);
            if let Some(c) = seg.poll(&mut Threshold)? {
                out.push(c.len());
            }
        }
        // O silêncio antes da segunda frase não entra no trecho.
        assert_eq!(out, [ms(800) + ms(200), ms(200) + ms(400) + ms(200)]);
        Ok(())
    }

    #[test]
    fn long_silence_is_dropped() -> Result<()> {
        let mut seg = Segmenter::default();
        seg.push(&silence(5_000));
        assert!(seg.poll(&mut Threshold)?.is_none());
        assert_eq!(seg.pending_len(), ms(200));
        Ok(())
    }

    #[test]
    fn nonstop_speech_is_cut_at_max_chunk() -> Result<()> {
        let mut seg = Segmenter::default();
        seg.push(&speech(26_000));
        assert!(seg.poll(&mut Threshold)?.is_some());
        Ok(())
    }

    #[test]
    fn click_too_short_is_not_transcribed() -> Result<()> {
        let mut seg = Segmenter::default();
        seg.push(&speech(100));
        seg.push(&silence(1_300));
        assert!(seg.poll(&mut Threshold)?.is_none());
        Ok(())
    }

    #[test]
    fn finish_returns_tail_with_speech() -> Result<()> {
        let mut seg = Segmenter::default();
        seg.push(&speech(600));
        assert!(seg.finish(&mut Threshold)?.is_some());
        let mut quiet = Segmenter::default();
        quiet.push(&silence(600));
        assert!(quiet.finish(&mut Threshold)?.is_none());
        Ok(())
    }
}
