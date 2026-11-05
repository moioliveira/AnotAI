//! Modelos do motor local: catálogo, download com verificação SHA-256 e o
//! modelo de VAD embutido no instalador.

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager};

use crate::pipeline;

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    file: &'static str,
    url: &'static str,
    sha256: &'static str,
    pub size: u64,
}

/// Medidos num notebook com Intel Iris Xe (só CPU), fala de ~10 s.
pub const MODELS: [ModelInfo; 2] = [
    ModelInfo {
        id: "base",
        label: "Rápido",
        description: "Whisper base · ~3 s por frase na CPU",
        file: "ggml-base-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin",
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        size: 59_707_625,
    },
    ModelInfo {
        id: "small",
        label: "Preciso",
        description: "Whisper small · ~10 s por frase na CPU",
        file: "ggml-small-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        size: 190_085_487,
    },
];

const VAD_FILE: &str = "ggml-silero-v6.2.0.bin";
pub const PROGRESS_EVENT: &str = "model-progress";

#[derive(Default)]
pub struct Downloads(Mutex<HashSet<&'static str>>);

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

fn models_dir() -> Option<PathBuf> {
    pipeline::data_dir().map(|d| d.join("models"))
}

pub fn path(model: &ModelInfo) -> Option<PathBuf> {
    models_dir().map(|d| d.join(model.file))
}

/// Instalado = arquivo presente com o tamanho esperado (o hash foi conferido
/// no download).
pub fn installed(model: &ModelInfo) -> bool {
    path(model)
        .and_then(|p| std::fs::metadata(p).ok())
        .is_some_and(|m| m.len() == model.size)
}

/// Modelo de VAD embutido no instalador.
pub fn vad_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resolve(VAD_FILE, BaseDirectory::Resource)
        .ok()
        .filter(|p| p.exists())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    size: u64,
    installed: bool,
    downloading: bool,
}

pub fn status(app: &AppHandle) -> Vec<ModelStatus> {
    let downloading = app
        .try_state::<Downloads>()
        .and_then(|d| d.0.lock().ok().map(|s| s.clone()))
        .unwrap_or_default();
    MODELS
        .iter()
        .map(|m| ModelStatus {
            id: m.id,
            label: m.label,
            description: m.description,
            size: m.size,
            installed: installed(m),
            downloading: downloading.contains(m.id),
        })
        .collect()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: &'static str,
    received: u64,
    total: u64,
}

/// Baixa o modelo para um `.part`, confere o SHA-256 e só então renomeia.
pub async fn download(app: &AppHandle, id: &str) -> Result<(), String> {
    let model = find(id).ok_or_else(|| format!("modelo desconhecido: {id}"))?;
    let downloads = app.state::<Downloads>();
    {
        let mut set = downloads.0.lock().map_err(|_| "estado corrompido")?;
        if !set.insert(model.id) {
            return Err("Este modelo já está sendo baixado.".into());
        }
    }
    let result = fetch(app, model).await;
    if let Ok(mut set) = downloads.0.lock() {
        set.remove(model.id);
    }
    result
}

async fn fetch(app: &AppHandle, model: &'static ModelInfo) -> Result<(), String> {
    let final_path = path(model).ok_or("APPDATA não definido")?;
    let dir = final_path.parent().ok_or("caminho inválido")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = final_path.with_extension("part");

    let mut resp = reqwest::Client::new()
        .get(model.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("falha ao baixar o modelo: {e}"))?;
    let total = resp.content_length().unwrap_or(model.size);
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    let outcome: Result<(), String> = async {
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| format!("download interrompido: {e}"))?
        {
            hasher.update(&chunk);
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            received += chunk.len() as u64;
            if last_emit.elapsed() >= Duration::from_millis(200) {
                last_emit = Instant::now();
                let _ = app.emit(
                    PROGRESS_EVENT,
                    Progress {
                        id: model.id,
                        received,
                        total,
                    },
                );
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        let digest: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if digest != model.sha256 {
            return Err("O arquivo baixado não confere (SHA-256 diferente). Tente de novo.".into());
        }
        Ok(())
    }
    .await;
    drop(file);

    if let Err(e) = outcome {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &final_path).map_err(|e| e.to_string())?;
    let _ = app.emit(
        PROGRESS_EVENT,
        Progress {
            id: model.id,
            received: total,
            total,
        },
    );
    log::info!("modelo {} instalado em {}", model.id, final_path.display());
    Ok(())
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    let model = find(id).ok_or_else(|| format!("modelo desconhecido: {id}"))?;
    crate::engine::unload_local(app);
    if let Some(p) = path(model)
        && p.exists()
    {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        for m in &MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.url.ends_with(m.file), "{}", m.id);
            assert!(m.size > 1_000_000, "{}", m.id);
        }
        assert!(find("base").is_some());
        assert!(find("large").is_none());
    }
}
