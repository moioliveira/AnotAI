//! Pipeline do ditado (Tech Spec 5.2): áudio → STT → pós-processamento →
//! inserção → histórico. Roda fora das threads do atalho e da UI.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anotai_audio::Recording;
use anotai_core::{AppState, CoreEvent, Mode};
use anotai_postproc::{LlmConfig, LlmOutcome, Options, Task, Term};
use anotai_stt::Language;
use tauri::{AppHandle, Manager};

use crate::hotkey::Session;
use crate::settings::Settings;
use crate::{engine, secrets, settings, ui};

/// Abaixo disso é toque acidental: nem chama o STT.
const MIN_DURATION: Duration = Duration::from_millis(400);
/// Pico abaixo disso é silêncio (microfone mudo ou sem fala).
const MIN_PEAK: f32 = 0.02;

/// Banco do histórico (ausente se não abriu).
pub struct Db(pub anotai_store::Store);

/// Um erro que o usuário precisa ver.
pub struct UserError {
    pub title: &'static str,
    pub body: String,
}

impl UserError {
    fn new(title: &'static str, body: impl Into<String>) -> Self {
        Self {
            title,
            body: body.into(),
        }
    }
}

/// Onde o trecho entra.
#[derive(Debug, Clone, Copy)]
pub struct ChunkInfo {
    /// `false` no primeiro trecho do modo contínuo: os seguintes ganham um
    /// espaço antes, para não grudar no anterior.
    pub follows_previous: bool,
}

/// Ditado inteiro (push-to-talk, ou contínuo sem VAD): muda o estado visível
/// e volta ao repouso no fim.
pub fn run(app: AppHandle, recording: anotai_audio::Result<Recording>, session: Session) {
    tauri::async_runtime::spawn(async move {
        let mode = session.mode;
        ui::set_state(&app, AppState::Transcribing, mode);
        let result = match recording {
            Ok(rec) => process(&app, rec, &session, None).await,
            Err(e) => Err(UserError::new("Falha na gravação", e.to_string())),
        };
        if let Err(err) = result {
            ui::notify(&app, err.title, &err.body);
            ui::set_state(&app, AppState::Error, mode);
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
        ui::set_state(&app, AppState::Idle, mode);
    });
}

/// Um trecho do modo contínuo: não mexe no estado (segue "Ouvindo").
pub async fn process_chunk(
    app: &AppHandle,
    recording: Recording,
    session: &Session,
    info: ChunkInfo,
) -> Result<(), UserError> {
    process(app, recording, session, Some(info)).await
}

async fn process(
    app: &AppHandle,
    recording: Recording,
    session: &Session,
    chunk: Option<ChunkInfo>,
) -> Result<(), UserError> {
    let cfg = settings::get(app);
    let duration = recording.duration();
    let peak = recording.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    log::info!("gravação: {:.2} s, pico {peak:.3}", duration.as_secs_f32());
    if cfg.save_recordings || std::env::var_os("ANOTAI_SAVE_RECORDINGS").is_some() {
        save_wav(&recording);
    }
    if duration < MIN_DURATION || peak < MIN_PEAK {
        log::info!("gravação curta ou silenciosa: ignorada");
        return Ok(());
    }

    // 1. Transcrição
    let started = Instant::now();
    let language = match session.mode {
        Mode::Dictation => Language::PtBr,
        Mode::Translation => Language::En,
    };
    let dictionary = load_dictionary(app).await;
    let vocabulary: Vec<String> = dictionary.iter().map(|t| t.term.clone()).collect();
    let transcript = engine::transcribe(app, &cfg, &recording, language, &vocabulary)
        .await
        .map_err(|e| {
            if e.needs_setup {
                crate::show_main_window(app);
            }
            UserError::new(e.title, e.body)
        })?;
    log::info!(
        "STT ({}) em {} ms",
        transcript.engine,
        started.elapsed().as_millis()
    );
    if transcript.text.trim().is_empty() {
        log::info!("transcrição vazia");
        return Ok(());
    }
    ui::emit(
        app,
        &CoreEvent::TranscriptPartial {
            text: transcript.text.clone(),
        },
    );

    // 2. Pós-processamento
    let text = postprocess(&cfg, session.mode, &transcript.text, dictionary).await;
    ui::emit(app, &CoreEvent::TranscriptFinal { text: text.clone() });

    // 3. Inserção
    if chunk.is_none() {
        ui::set_state(app, AppState::Inserting, session.mode);
    }
    let to_insert = match chunk {
        Some(ChunkInfo {
            follows_previous: true,
        }) => format!(" {text}"),
        _ => text.clone(),
    };
    // No modo contínuo o texto vai para onde o cursor estiver agora.
    let target = if chunk.is_some() {
        None
    } else {
        session.target
    };
    let method = cfg.insert_method;
    let inserted =
        tokio::task::spawn_blocking(move || anotai_inject::insert(&to_insert, target, method))
            .await
            .map_err(|e| UserError::new("Falha ao inserir", e.to_string()))?;
    let latency_ms = started.elapsed().as_millis() as i64;
    log::info!("inserção: {inserted:?} em {latency_ms} ms desde o fim da fala");
    let insert_error = insert_error(&inserted, &text);

    // 4. Histórico
    if cfg.history.retention_days > 0
        && let Some(db) = app.try_state::<Db>()
    {
        let row = anotai_store::NewTranscription {
            mode: mode_id(session.mode).into(),
            trigger: session.trigger.into(),
            raw_text: transcript.text,
            final_text: text,
            source_lang: language.bcp47().into(),
            target_lang: "pt-BR".into(),
            target_app: session.target_app.clone(),
            inserted: inserted.is_ok(),
            duration_ms: duration.as_millis() as i64,
            latency_ms,
            engine: transcript.engine,
        };
        match db.0.add_transcription(&row).await {
            Ok(_) => ui::emit(app, &CoreEvent::HistoryChanged),
            Err(e) => log::warn!("falha ao gravar no histórico: {e}"),
        }
    }

    match insert_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

async fn postprocess(cfg: &Settings, mode: Mode, raw: &str, dictionary: Vec<Term>) -> String {
    let options = Options {
        task: match mode {
            Mode::Dictation => Task::Correct,
            Mode::Translation => Task::Translate,
        },
        remove_fillers: cfg.postproc.remove_fillers,
        correction: cfg.postproc.correction,
        dictionary,
        // A correção também manda texto para a nuvem: exige consentimento.
        llm: secrets::get("anthropic")
            .filter(|_| cfg.cloud_consent_at.is_some())
            .map(|k| LlmConfig {
                model: cfg.postproc.llm_model.clone(),
                ..LlmConfig::new(k)
            }),
    };
    let processed = anotai_postproc::process(raw, &options).await;
    if let LlmOutcome::Failed(reason) = &processed.llm {
        log::warn!("correção indisponível ({reason}); texto inserido só com as regras");
    }
    processed.text
}

fn insert_error(
    inserted: &Result<anotai_inject::Inserted, anotai_inject::Error>,
    text: &str,
) -> Option<UserError> {
    match inserted {
        Ok(_) => None,
        Err(anotai_inject::Error::PasswordField) => Some(UserError::new(
            "Campo de senha",
            "Por segurança, o AnotAI não insere texto em campos de senha.",
        )),
        Err(err) => {
            let copied = anotai_inject::copy_to_clipboard(text).is_ok();
            Some(UserError::new(
                "Não consegui inserir o texto",
                if copied {
                    format!("{err}. O texto está na área de transferência: cole com Ctrl+V.")
                } else {
                    err.to_string()
                },
            ))
        }
    }
}

async fn load_dictionary(app: &AppHandle) -> Vec<Term> {
    let Some(db) = app.try_state::<Db>() else {
        return vec![];
    };
    match db.0.dictionary().await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| Term {
                term: r.term,
                spoken_as: r.spoken_as,
            })
            .collect(),
        Err(e) => {
            log::warn!("falha ao ler o dicionário: {e}");
            vec![]
        }
    }
}

pub fn mode_id(mode: Mode) -> &'static str {
    match mode {
        Mode::Dictation => "dictation",
        Mode::Translation => "translation",
    }
}

/// Pasta de dados (`%APPDATA%\AnotAI`, Tech Spec 6).
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("AnotAI"))
}

fn save_wav(recording: &Recording) {
    let Some(dir) = data_dir().map(|d| d.join("recordings")) else {
        return;
    };
    let name = format!(
        "{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    );
    let path = dir.join(name);
    let result = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::File::create(&path))
        .and_then(|f| recording.write_wav(std::io::BufWriter::new(f)));
    match result {
        Ok(()) => log::info!("gravação salva em {}", path.display()),
        Err(err) => log::warn!("falha ao salvar gravação: {err}"),
    }
}
