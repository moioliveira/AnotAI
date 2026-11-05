//! Modo contínuo com VAD: a cada pausa, a frase é transcrita e inserida sem
//! fechar o microfone. Sem VAD, o modo contínuo cai no comportamento antigo
//! (tudo sai ao encerrar).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anotai_audio::{Recorder, Recording, TARGET_RATE};
use anotai_core::AppState;
use anotai_vad::{Segmenter, SegmenterConfig, SileroVad};
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::hotkey::Session;
use crate::pipeline::{self, ChunkInfo};
use crate::{engine, ui};

const POLL: Duration = Duration::from_millis(300);

/// Sinal para encerrar o modo contínuo.
pub struct Handle(oneshot::Sender<()>);

impl Handle {
    pub fn stop(self) {
        let _ = self.0.send(());
    }
}

/// Inicia o laço; `None` se não houver VAD (o chamador usa o modo antigo).
pub fn start(app: &AppHandle, recorder: Arc<Recorder>, session: Session) -> Option<Handle> {
    let vad = engine::vad(app)?;
    let (tx, rx) = oneshot::channel();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run(&app, recorder, vad, session, rx).await;
    });
    Some(Handle(tx))
}

fn to_recording(samples: Vec<f32>) -> Recording {
    Recording {
        samples,
        sample_rate: TARGET_RATE,
    }
}

/// Roda o VAD fora do executor (custa alguns ms por chamada).
async fn poll(segmenter: Segmenter, vad: &Arc<Mutex<SileroVad>>) -> (Segmenter, Option<Vec<f32>>) {
    let vad = Arc::clone(vad);
    let joined = tokio::task::spawn_blocking(move || {
        let mut segmenter = segmenter;
        let chunk = match vad.lock() {
            Ok(mut v) => segmenter.poll(&mut *v),
            Err(_) => Ok(None),
        };
        (segmenter, chunk)
    })
    .await;
    match joined {
        Ok((s, Ok(chunk))) => (s, chunk),
        Ok((s, Err(e))) => {
            log::warn!("VAD falhou: {e}");
            (s, None)
        }
        Err(e) => {
            log::error!("tarefa do VAD falhou: {e}");
            (Segmenter::new(SegmenterConfig::default()), None)
        }
    }
}

async fn run(
    app: &AppHandle,
    recorder: Arc<Recorder>,
    vad: Arc<Mutex<SileroVad>>,
    session: Session,
    mut stop: oneshot::Receiver<()>,
) {
    let mut segmenter = Segmenter::new(SegmenterConfig::default());
    let mut chunks_done = 0usize;
    let mut notified = false;

    loop {
        let stopping = tokio::select! {
            _ = &mut stop => true,
            () = tokio::time::sleep(POLL) => false,
        };
        // No fim, `finish` fecha o microfone e entrega o resto.
        let rec = Arc::clone(&recorder);
        let audio =
            tokio::task::spawn_blocking(move || if stopping { rec.finish() } else { rec.drain() })
                .await;
        match audio {
            Ok(Ok(r)) => segmenter.push(&r.samples),
            Ok(Err(e)) => log::warn!("falha ao ler o microfone: {e}"),
            Err(e) => log::error!("tarefa de áudio falhou: {e}"),
        }

        let (next, chunk) = poll(segmenter, &vad).await;
        segmenter = next;
        let mut ready: Vec<Vec<f32>> = chunk.into_iter().collect();
        if stopping {
            let tail = match vad.lock() {
                Ok(mut v) => std::mem::take(&mut segmenter).finish(&mut *v),
                Err(_) => Ok(None),
            };
            match tail {
                Ok(Some(t)) => ready.push(t),
                Ok(None) => {}
                Err(e) => log::warn!("VAD falhou no fim: {e}"),
            }
        }

        // Um trecho de cada vez, na ordem em que foi falado.
        for samples in ready {
            if stopping {
                ui::set_state(app, AppState::Transcribing, session.mode);
            }
            let info = ChunkInfo {
                follows_previous: chunks_done > 0,
            };
            match pipeline::process_chunk(app, to_recording(samples), &session, info).await {
                Ok(()) => chunks_done += 1,
                Err(err) if !notified => {
                    // Avisa uma vez; repetir a cada frase seria só ruído.
                    notified = true;
                    ui::notify(app, err.title, &err.body);
                }
                Err(err) => log::warn!("{}: {}", err.title, err.body),
            }
        }
        if stopping {
            break;
        }
    }
    log::info!("modo contínuo encerrado: {chunks_done} trechos inseridos");
    ui::set_state(app, AppState::Idle, session.mode);
}
