//! Gravador: uma thread dona do stream do cpal recebe comandos por canal.

use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::dsp::{downmix_into, resample, rms};
use crate::{Error, MAX_RECORDING, Recording, Result, TARGET_RATE};

/// Recebe o nível do microfone (RMS) para o medidor do overlay.
pub type LevelCallback = Arc<dyn Fn(f32) + Send + Sync>;

/// Frequência máxima das chamadas de nível.
const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

enum Command {
    Arm,
    Finish(SyncSender<Result<Recording>>),
    /// Entrega o áudio até agora e continua gravando (modo contínuo).
    Drain(SyncSender<Result<Recording>>),
    Discard,
}

/// Gravador do microfone padrão. Barato quando ocioso: o dispositivo só abre
/// em [`Recorder::arm`].
pub struct Recorder {
    tx: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl Recorder {
    pub fn spawn(on_level: LevelCallback) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("anotai-audio".into())
            .spawn(move || run(rx, on_level))
            .map_err(|e| Error::Device(e.to_string()))?;
        Ok(Self {
            tx: Some(tx),
            worker: Some(worker),
        })
    }

    /// Abre o microfone e começa a guardar o áudio. Ignorado se já gravando.
    pub fn arm(&self) {
        self.send(Command::Arm);
    }

    /// Para a gravação e devolve o áudio em 16 kHz mono.
    pub fn finish(&self) -> Result<Recording> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.tx
            .as_ref()
            .ok_or(Error::WorkerGone)?
            .send(Command::Finish(reply_tx))
            .map_err(|_| Error::WorkerGone)?;
        reply_rx.recv().map_err(|_| Error::WorkerGone)?
    }

    /// Entrega o áudio gravado desde a última chamada, sem fechar o
    /// microfone (modo contínuo).
    pub fn drain(&self) -> Result<Recording> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.tx
            .as_ref()
            .ok_or(Error::WorkerGone)?
            .send(Command::Drain(reply_tx))
            .map_err(|_| Error::WorkerGone)?;
        reply_rx.recv().map_err(|_| Error::WorkerGone)?
    }

    /// Fecha o microfone e joga o áudio fora.
    pub fn discard(&self) {
        self.send(Command::Discard);
    }

    fn send(&self, cmd: Command) {
        if let Some(tx) = &self.tx
            && tx.send(cmd).is_err()
        {
            tracing::warn!("a thread de áudio parou");
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // Fechar o canal encerra a thread.
        self.tx.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::warn!("a thread de áudio terminou com pânico");
        }
    }
}

/// Gravação em andamento.
struct Active {
    // O stream fecha o dispositivo ao ser descartado.
    _stream: Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

fn run(rx: Receiver<Command>, on_level: LevelCallback) {
    let mut active: Option<Active> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::Arm => {
                if active.is_none() {
                    match open(on_level.clone()) {
                        Ok(a) => active = Some(a),
                        Err(e) => tracing::error!("não foi possível abrir o microfone: {e}"),
                    }
                }
            }
            Command::Finish(reply) => {
                let result = match active.take() {
                    Some(a) => close(a),
                    None => Err(Error::NotArmed),
                };
                on_level(0.0);
                let _ = reply.send(result);
            }
            Command::Drain(reply) => {
                let result = match &active {
                    Some(a) => take_resampled(&a.buffer, a.sample_rate),
                    None => Err(Error::NotArmed),
                };
                let _ = reply.send(result);
            }
            Command::Discard => {
                if active.take().is_some() {
                    on_level(0.0);
                }
            }
        }
    }
}

fn open(on_level: LevelCallback) -> Result<Active> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(Error::NoInputDevice)?;
    let supported = device
        .default_input_config()
        .map_err(|e| Error::Device(e.to_string()))?;
    let config: StreamConfig = supported.config();
    let sample_rate = config.sample_rate;
    let channels = usize::from(config.channels);
    let max_samples = (MAX_RECORDING.as_secs() as usize) * sample_rate as usize;
    let buffer = Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 10)));

    let stream = match supported.sample_format() {
        SampleFormat::F32 => {
            build::<f32>(&device, config, channels, &buffer, max_samples, on_level)
        }
        SampleFormat::I16 => {
            build::<i16>(&device, config, channels, &buffer, max_samples, on_level)
        }
        SampleFormat::I32 => {
            build::<i32>(&device, config, channels, &buffer, max_samples, on_level)
        }
        SampleFormat::U16 => {
            build::<u16>(&device, config, channels, &buffer, max_samples, on_level)
        }
        other => return Err(Error::UnsupportedFormat(format!("{other:?}"))),
    }?;
    stream.play().map_err(|e| Error::Device(e.to_string()))?;
    tracing::info!(sample_rate, channels, "microfone aberto");
    Ok(Active {
        _stream: stream,
        buffer,
        sample_rate,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    channels: usize,
    buffer: &Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
    on_level: LevelCallback,
) -> Result<Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let buffer = Arc::clone(buffer);
    let mut scratch = Vec::with_capacity(4096);
    let mut last_level = Instant::now();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                scratch.clear();
                downmix_into(data, channels, &mut scratch);
                if let Ok(mut buf) = buffer.lock()
                    && buf.len() < max_samples
                {
                    buf.extend_from_slice(&scratch);
                }
                if last_level.elapsed() >= LEVEL_INTERVAL {
                    last_level = Instant::now();
                    on_level(rms(&scratch));
                }
            },
            |err| tracing::error!("erro no stream do microfone: {err}"),
            None,
        )
        .map_err(|e| Error::Device(e.to_string()))
}

/// Esvazia o buffer e converte para 16 kHz.
fn take_resampled(buffer: &Mutex<Vec<f32>>, sample_rate: u32) -> Result<Recording> {
    let raw = buffer
        .lock()
        .map(|mut b| std::mem::take(&mut *b))
        .map_err(|_| Error::Device("buffer de áudio corrompido".into()))?;
    let samples = resample(&raw, sample_rate, TARGET_RATE)?;
    Ok(Recording {
        samples,
        sample_rate: TARGET_RATE,
    })
}

fn close(active: Active) -> Result<Recording> {
    // Para o stream antes de ler o buffer: nada mais chega depois.
    let Active {
        _stream: stream,
        buffer,
        sample_rate,
    } = active;
    drop(stream);
    take_resampled(&buffer, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem tocar no microfone: finalizar sem armar dá erro claro.
    #[test]
    fn finish_without_arm_is_not_armed() -> Result<()> {
        let rec = Recorder::spawn(Arc::new(|_| {}))?;
        assert!(matches!(rec.finish(), Err(Error::NotArmed)));
        rec.discard();
        Ok(())
    }
}
