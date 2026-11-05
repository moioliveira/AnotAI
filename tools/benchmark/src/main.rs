//! Benchmark de transcrição (Sprint 0, ADR-003).
//!
//! ```text
//! cargo run --release -p anotai-benchmark -- \
//!     --dataset tools/benchmark/dataset \
//!     --engines groq,openai,deepgram,local:base,local:small \
//!     [--models-dir %APPDATA%\AnotAI\models] [--price groq=0.04] [--csv resultados.csv]
//! ```
//!
//! O dataset tem uma pasta por idioma (`pt-BR/`, `en/`), com pares
//! `nome.wav` + `nome.txt` (transcrição de referência). Chaves dos provedores
//! de nuvem vêm do ambiente: `GROQ_API_KEY`, `OPENAI_API_KEY`,
//! `DEEPGRAM_API_KEY`. Preço (`--price provedor=US$ por hora de áudio`) é
//! opcional: confira o valor atual no site de cada provedor.

mod wer;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anotai_audio::Recording;
use anotai_stt::{CloudProvider, Language, LocalWhisper, SttProvider};

struct Sample {
    name: String,
    language: Language,
    recording: Recording,
    reference: String,
}

#[derive(Default)]
struct Totals {
    errors: usize,
    words: usize,
    audio_secs: f64,
    latencies_ms: Vec<u128>,
    failures: usize,
}

struct Args {
    dataset: PathBuf,
    engines: Vec<String>,
    models_dir: PathBuf,
    prices: BTreeMap<String, f64>,
    csv: Option<PathBuf>,
}

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn parse_args() -> AnyResult<Args> {
    let mut args = Args {
        dataset: PathBuf::from("tools/benchmark/dataset"),
        engines: vec!["local:base".into()],
        models_dir: std::env::var_os("APPDATA")
            .map(|d| PathBuf::from(d).join("AnotAI").join("models"))
            .unwrap_or_default(),
        prices: BTreeMap::new(),
        csv: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} precisa de um valor"))
        };
        match flag.as_str() {
            "--dataset" => args.dataset = value()?.into(),
            "--engines" => args.engines = value()?.split(',').map(str::to_owned).collect(),
            "--models-dir" => args.models_dir = value()?.into(),
            "--csv" => args.csv = Some(value()?.into()),
            "--price" => {
                let v = value()?;
                let (k, p) = v.split_once('=').ok_or("use --price provedor=valor")?;
                args.prices.insert(k.to_owned(), p.parse()?);
            }
            "-h" | "--help" => {
                println!(
                    "{}",
                    include_str!("main.rs")
                        .lines()
                        .take(17)
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}").into()),
        }
    }
    Ok(args)
}

fn load_dataset(dir: &Path) -> AnyResult<Vec<Sample>> {
    let mut samples = Vec::new();
    for (folder, language) in [("pt-BR", Language::PtBr), ("en", Language::En)] {
        let Ok(entries) = std::fs::read_dir(dir.join(folder)) else {
            continue;
        };
        let mut wavs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
            .collect();
        wavs.sort();
        for wav in wavs {
            let txt = wav.with_extension("txt");
            let Ok(reference) = std::fs::read_to_string(&txt) else {
                eprintln!("aviso: sem referência para {}", wav.display());
                continue;
            };
            samples.push(Sample {
                name: format!(
                    "{folder}/{}",
                    wav.file_stem().unwrap_or_default().to_string_lossy()
                ),
                language,
                recording: Recording::read_wav(&std::fs::read(&wav)?)?,
                // Arquivos salvos pelo PowerShell/Notepad podem ter BOM.
                reference: reference.trim_start_matches('\u{feff}').trim().to_owned(),
            });
        }
    }
    if samples.is_empty() {
        return Err(format!("nenhum par .wav/.txt em {}/{{pt-BR,en}}", dir.display()).into());
    }
    Ok(samples)
}

fn build_engine(spec: &str, models_dir: &Path) -> AnyResult<Box<dyn SttProvider>> {
    if let Some(model) = spec.strip_prefix("local:") {
        let file = match model {
            "tiny" => "ggml-tiny-q5_1.bin",
            "base" => "ggml-base-q5_1.bin",
            "small" => "ggml-small-q5_1.bin",
            "large-v3-turbo" => "ggml-large-v3-turbo-q5_0.bin",
            other => return Err(format!("modelo local desconhecido: {other}").into()),
        };
        let path = models_dir.join(file);
        if !path.exists() {
            return Err(format!("modelo não encontrado: {}", path.display()).into());
        }
        return Ok(Box::new(LocalWhisper::load(&path, model, None)?));
    }
    let (provider, var) = match spec {
        "openai" => (CloudProvider::OpenAi, "OPENAI_API_KEY"),
        "groq" => (CloudProvider::Groq, "GROQ_API_KEY"),
        "deepgram" => (CloudProvider::Deepgram, "DEEPGRAM_API_KEY"),
        other => return Err(format!("motor desconhecido: {other}").into()),
    };
    let key = std::env::var(var).map_err(|_| format!("defina {var} para testar {spec}"))?;
    Ok(provider.client(key))
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let args = parse_args()?;
    let samples = load_dataset(&args.dataset)?;
    let audio_total: f64 = samples
        .iter()
        .map(|s| s.recording.duration().as_secs_f64())
        .sum();
    eprintln!(
        "{} amostras, {:.1} min de áudio",
        samples.len(),
        audio_total / 60.0
    );

    let mut csv = String::from("engine,sample,errors,words,latency_ms,hypothesis\n");
    let mut results: Vec<(String, Totals)> = Vec::new();

    for spec in &args.engines {
        let engine = match build_engine(spec, &args.models_dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("pulando {spec}: {e}");
                continue;
            }
        };
        let mut totals = Totals::default();
        for s in &samples {
            let started = Instant::now();
            match engine.transcribe(&s.recording, s.language).await {
                Ok(t) => {
                    let ms = started.elapsed().as_millis();
                    let (errors, words) = wer::wer_counts(&s.reference, &t.text);
                    totals.errors += errors;
                    totals.words += words;
                    totals.audio_secs += s.recording.duration().as_secs_f64();
                    totals.latencies_ms.push(ms);
                    csv.push_str(&format!(
                        "{spec},{},{errors},{words},{ms},\"{}\"\n",
                        s.name,
                        t.text.replace('"', "\"\"")
                    ));
                    eprintln!("{spec} {}: {errors}/{words} erros, {ms} ms", s.name);
                }
                Err(e) => {
                    totals.failures += 1;
                    eprintln!("{spec} {}: falhou: {e}", s.name);
                }
            }
        }
        results.push((spec.clone(), totals));
    }

    println!("| Motor | WER | Latência p50 | Latência p95 | Falhas | US$/hora |");
    println!("|---|---:|---:|---:|---:|---:|");
    for (spec, mut t) in results {
        t.latencies_ms.sort_unstable();
        let wer = if t.words == 0 {
            0.0
        } else {
            t.errors as f64 / t.words as f64 * 100.0
        };
        let price = args
            .prices
            .get(spec.as_str())
            .map_or_else(|| "—".to_owned(), |p| format!("{p:.3}"));
        println!(
            "| {spec} | {wer:.1}% | {} ms | {} ms | {} | {price} |",
            percentile(&t.latencies_ms, 0.5),
            percentile(&t.latencies_ms, 0.95),
            t.failures
        );
    }

    if let Some(path) = args.csv {
        std::fs::write(&path, csv)?;
        eprintln!("detalhes em {}", path.display());
    }
    Ok(())
}
