//! Usa o microfone de verdade. Não roda por padrão:
//! `cargo test -p anotai-audio --test microphone -- --ignored --nocapture`

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anotai_audio::{Recorder, TARGET_RATE};

#[test]
#[ignore = "liga o microfone"]
fn records_one_second_from_default_microphone() -> anotai_audio::Result<()> {
    let levels = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&levels);
    let rec = Recorder::spawn(Arc::new(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    }))?;
    rec.arm();
    std::thread::sleep(Duration::from_secs(1));
    let recording = rec.finish()?;
    eprintln!(
        "{:.2} s, {} amostras, {} leituras de nível",
        recording.duration().as_secs_f32(),
        recording.samples.len(),
        levels.load(Ordering::Relaxed)
    );
    assert_eq!(recording.sample_rate, TARGET_RATE);
    // Abrir o dispositivo leva alguns ms; aceita a partir de 0,7 s.
    assert!(recording.duration() > Duration::from_millis(700));
    assert!(levels.load(Ordering::Relaxed) > 5);
    Ok(())
}
