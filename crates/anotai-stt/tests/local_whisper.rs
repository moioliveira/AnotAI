//! Transcreve WAVs reais com whisper.cpp. Não roda por padrão:
//!
//! ```text
//! ANOTAI_TEST_MODEL=caminho/ggml-tiny-q5_1.bin \
//! ANOTAI_TEST_VAD=caminho/ggml-silero-v6.2.0.bin \
//! ANOTAI_TEST_WAV_DIR=pasta-com-pt1.wav-e-en1.wav \
//! cargo test -p anotai-stt --test local_whisper -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::time::Instant;

use anotai_audio::Recording;
use anotai_stt::{Language, LocalWhisper, SttProvider};

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
#[ignore = "precisa de modelo e WAVs (ver doc do arquivo)"]
async fn transcribes_portuguese_and_english() -> Result<(), Box<dyn std::error::Error>> {
    let (Some(model), Some(dir)) = (
        env_path("ANOTAI_TEST_MODEL"),
        env_path("ANOTAI_TEST_WAV_DIR"),
    ) else {
        return Err("defina ANOTAI_TEST_MODEL e ANOTAI_TEST_WAV_DIR".into());
    };
    eprintln!("whisper.cpp: {}", whisper_rs::print_system_info());
    let started = Instant::now();
    let whisper = LocalWhisper::load(&model, "test", env_path("ANOTAI_TEST_VAD"))?;
    eprintln!("modelo carregado em {} ms", started.elapsed().as_millis());

    for (file, lang, expected) in [
        (
            "pt1.wav",
            Language::PtBr,
            ["relatório", "vendas", "reunião"],
        ),
        ("en1.wav", Language::En, ["report", "meeting", "thursday"]),
    ] {
        let rec = Recording::read_wav(&std::fs::read(dir.join(file))?)?;
        let t0 = Instant::now();
        let out = whisper.transcribe(&rec, lang).await?;
        eprintln!(
            "{file}: {:.1} s de áudio em {} ms → {:?} ({})",
            rec.duration().as_secs_f32(),
            t0.elapsed().as_millis(),
            out.text,
            out.engine
        );
        let got = words(&out.text);
        let hits = expected
            .iter()
            .filter(|w| got.contains(&w.to_string()))
            .count();
        assert!(
            hits >= 2,
            "esperava ao menos 2 de {expected:?} em {:?}",
            out.text
        );
    }
    Ok(())
}
