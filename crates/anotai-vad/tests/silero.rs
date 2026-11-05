//! Silero de verdade sobre fala sintetizada. Não roda por padrão:
//! `ANOTAI_TEST_VAD=... ANOTAI_TEST_WAV_DIR=... cargo test -p anotai-vad --test silero -- --ignored --nocapture`

use std::path::PathBuf;

use anotai_audio::Recording;
use anotai_vad::{SAMPLE_RATE, Segmenter, SegmenterConfig, SileroVad, SpeechDetector};

#[test]
#[ignore = "precisa do modelo de VAD e de um WAV"]
fn finds_speech_and_splits_on_pause() -> Result<(), Box<dyn std::error::Error>> {
    let vad_path = PathBuf::from(std::env::var("ANOTAI_TEST_VAD")?);
    let dir = PathBuf::from(std::env::var("ANOTAI_TEST_WAV_DIR")?);
    let mut vad = SileroVad::load(&vad_path)?;
    let speech = Recording::read_wav(&std::fs::read(dir.join("pt1.wav"))?)?.samples;

    let segs = vad.speech_segments(&speech)?;
    eprintln!("{} segmentos: {segs:?}", segs.len());
    assert!(!segs.is_empty());

    // Frase, 1,5 s de silêncio, frase, 1,5 s de silêncio: dois trechos,
    // alimentando em blocos de 250 ms como o microfone faria.
    let mut seg = Segmenter::new(SegmenterConfig::default());
    let mut chunks = Vec::new();
    let silence = vec![0.0; SAMPLE_RATE * 3 / 2];
    for part in [&speech, &silence, &speech, &silence] {
        for block in part.chunks(SAMPLE_RATE / 4) {
            seg.push(block);
            if let Some(c) = seg.poll(&mut vad)? {
                chunks.push(c.len() as f32 / SAMPLE_RATE as f32);
            }
        }
    }
    eprintln!("trechos (s): {chunks:?}");
    assert_eq!(chunks.len(), 2);
    Ok(())
}
