//! Captura de áudio do microfone padrão, mixagem para mono e reamostragem
//! para 16 kHz (formato que os motores de STT esperam).
//!
//! O microfone só abre quando o atalho é pressionado ([`Recorder::arm`]), não
//! fica ligado o tempo todo: assim o indicador de microfone do Windows só
//! acende durante o ditado. Abrir no key-down, antes de saber se é toque ou
//! "segurar", cumpre o papel do pré-buffer de 300 ms do Tech Spec (5.2).

mod dsp;
mod recorder;
mod wav;

pub use dsp::{downmix_into, resample, rms};
pub use recorder::{LevelCallback, Recorder};

use std::time::Duration;

/// Taxa de saída para o STT.
pub const TARGET_RATE: u32 = 16_000;

/// Teto de uma gravação (modo contínuo esquecido ligado). O VAD vai
/// segmentar antes disso quando existir.
pub const MAX_RECORDING: Duration = Duration::from_secs(10 * 60);

/// Áudio pronto para o STT: mono, `f32` em [-1, 1], 16 kHz.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Recording {
    pub fn duration(&self) -> Duration {
        if self.sample_rate == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.samples.len() as f64 / f64::from(self.sample_rate))
    }
}

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("nenhum microfone encontrado")]
    NoInputDevice,
    #[error("falha no microfone: {0}")]
    Device(String),
    #[error("formato de áudio não suportado: {0}")]
    UnsupportedFormat(String),
    #[error("falha ao reamostrar: {0}")]
    Resample(String),
    #[error("não há gravação em andamento")]
    NotArmed,
    #[error("a thread de áudio parou")]
    WorkerGone,
}

pub type Result<T> = std::result::Result<T, Error>;
