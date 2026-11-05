//! Endpoint `/audio/transcriptions` no formato da OpenAI (OpenAI e Groq).

use anotai_audio::Recording;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use crate::{
    BoxFuture, Error, Language, Result, SttProvider, Transcript, check_status, http_client,
    network, wav_bytes,
};

/// Adaptador para APIs compatíveis com a OpenAI.
#[derive(Debug, Clone)]
pub struct OpenAiCompatible {
    id: &'static str,
    base_url: String,
    model: String,
    api_key: String,
}

#[derive(Deserialize)]
struct Response {
    text: String,
}

impl OpenAiCompatible {
    pub fn openai(api_key: String) -> Self {
        Self {
            id: "openai",
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-transcribe".into(),
            api_key,
        }
    }

    pub fn groq(api_key: String) -> Self {
        Self {
            id: "groq",
            base_url: "https://api.groq.com/openai/v1".into(),
            model: "whisper-large-v3-turbo".into(),
            api_key,
        }
    }

    /// Troca a URL base (testes, proxy da v1.1).
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    async fn run(&self, recording: &Recording, language: Language) -> Result<Transcript> {
        if self.api_key.is_empty() {
            return Err(Error::MissingKey(self.id));
        }
        let file = Part::bytes(wav_bytes(recording)?)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(network)?;
        let form = Form::new()
            .part("file", file)
            .text("model", self.model.clone())
            .text("language", language.iso639_1())
            .text("response_format", "json");

        let resp = http_client()?
            .post(format!("{}/audio/transcriptions", self.base_url))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(network)?;
        let body: Response = check_status(resp)
            .await?
            .json()
            .await
            .map_err(|e| Error::Parse(e.to_string()))?;
        Ok(Transcript {
            text: body.text.trim().to_owned(),
            engine: self.engine_id(),
        })
    }
}

impl SttProvider for OpenAiCompatible {
    fn engine_id(&self) -> String {
        format!("cloud:{}", self.id)
    }

    fn transcribe<'a>(
        &'a self,
        recording: &'a Recording,
        language: Language,
    ) -> BoxFuture<'a, Result<Transcript>> {
        Box::pin(self.run(recording, language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    fn rec() -> Recording {
        Recording {
            samples: vec![0.0; 1600],
            sample_rate: 16_000,
        }
    }

    #[tokio::test]
    async fn sends_multipart_and_reads_text() -> Result<()> {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", "Bearer k-123")
            .match_header("content-type", Matcher::Regex("multipart/form-data".into()))
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("name=\"model\"\r\n\r\nwhisper-large-v3-turbo".into()),
                Matcher::Regex("name=\"language\"\r\n\r\npt".into()),
                Matcher::Regex("RIFF".into()),
            ]))
            .with_header("content-type", "application/json")
            .with_body(r#"{"text":"  olá mundo  "}"#)
            .create_async()
            .await;

        let stt = OpenAiCompatible::groq("k-123".into()).with_base_url(server.url());
        let t = stt.transcribe(&rec(), Language::PtBr).await?;
        assert_eq!(t.text, "olá mundo");
        assert_eq!(t.engine, "cloud:groq");
        mock.assert_async().await;
        Ok(())
    }

    #[tokio::test]
    async fn maps_401_to_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/audio/transcriptions")
            .with_status(401)
            .create_async()
            .await;
        let stt = OpenAiCompatible::openai("x".into()).with_base_url(server.url());
        assert!(matches!(
            stt.transcribe(&rec(), Language::En).await,
            Err(Error::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn empty_key_fails_fast() {
        let stt = OpenAiCompatible::openai(String::new());
        assert!(matches!(
            stt.transcribe(&rec(), Language::En).await,
            Err(Error::MissingKey("openai"))
        ));
    }
}
