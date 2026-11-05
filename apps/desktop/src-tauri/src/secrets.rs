//! Chaves de API no Windows Credential Manager (Tech Spec 6.3). A UI só
//! grava e consulta se existe; nunca lê a chave de volta.
//!
//! Em builds de debug, variáveis de ambiente (`GROQ_API_KEY` etc., vindas do
//! `.env.local`) servem de alternativa.

use std::collections::BTreeMap;

const SERVICE: &str = "AnotAI";

/// Provedores com chave: os de STT e a Anthropic (correção/tradução).
pub const PROVIDERS: [(&str, &str); 4] = [
    ("openai", "OPENAI_API_KEY"),
    ("groq", "GROQ_API_KEY"),
    ("deepgram", "DEEPGRAM_API_KEY"),
    ("anthropic", "ANTHROPIC_API_KEY"),
];

fn env_var_for(id: &str) -> Option<&'static str> {
    PROVIDERS.iter().find(|(p, _)| *p == id).map(|(_, v)| *v)
}

pub fn get(id: &str) -> Option<String> {
    let stored = keyring::Entry::new(SERVICE, id)
        .and_then(|e| e.get_password())
        .ok()
        .filter(|k| !k.trim().is_empty());
    stored.or_else(|| {
        if !cfg!(debug_assertions) {
            return None;
        }
        env_var_for(id)
            .and_then(|v| std::env::var(v).ok())
            .filter(|k| !k.trim().is_empty())
    })
}

/// Grava a chave; vazia apaga.
pub fn set(id: &str, key: &str) -> Result<(), String> {
    if env_var_for(id).is_none() {
        return Err(format!("provedor desconhecido: {id}"));
    }
    let entry = keyring::Entry::new(SERVICE, id).map_err(|e| e.to_string())?;
    let key = key.trim();
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry.set_password(key).map_err(|e| e.to_string())
    }
}

/// Quais provedores têm chave configurada.
pub fn status() -> BTreeMap<&'static str, bool> {
    PROVIDERS
        .iter()
        .map(|(id, _)| (*id, get(id).is_some()))
        .collect()
}

/// Remove todas as chaves ("Apagar tudo").
pub fn clear_all() {
    for (id, _) in PROVIDERS {
        if let Err(e) = set(id, "") {
            log::warn!("falha ao apagar a chave de {id}: {e}");
        }
    }
}
