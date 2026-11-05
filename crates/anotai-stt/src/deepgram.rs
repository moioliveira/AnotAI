//! Deepgram Nova-3 (áudio pré-gravado).

use anotai_audio::Recording;
use serde::Deserialize;

use crate::{
    BoxFuture, Error, Language, Result, SttProvider, Transcript, check_status, http_client,
    network, wav_bytes,
};

#[derive(Debug, Clone)]
pub struct Deepgram {
    base_url: String,
    model: String,
    api_key: String,
}

#[derive(Deserialize)]
struct Response {
    results: Results,
}
#[derive(Deserialize)]
struct Results {
    channels: Vec<Channel>,
}
#[derive(Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}
#[derive(Deserialize)]
struct Alternative {
    transcript: String,
}

impl Deepgram {
    pub fn new(api_key: String) -> Self {
        Self {
            base_url: "https://api.deepgram.com/v1".into(),
            model: "nova-3".into(),
            api_key,
        }
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn run(&self, recording: &Recording, language: Language) -> Result<Transcript> {
        if self.api_key.is_empty() {
            return Err(Error::MissingKey("deepgram"));
        }
        let resp = http_client()?
            .post(format!("{}/listen", self.base_url))
            .query(&[
                ("model", self.model.as_str()),
                ("language", language.bcp47()),
                ("smart_format", "true"),
                ("punctuate", "true"),
            ])
            .header("authorization", format!("Token {}", self.api_key))
            .header("content-type", "audio/wav")
            .body(wav_bytes(recording)?)
            .send()
            .await
            .map_err(network)?;
        let body: Response = check_status(resp)
            .await?
            .json()
            .await
            .map_err(|e| Error::Parse(e.to_string()))?;
        let text = body
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|c| c.alternatives.into_iter().next())
            .map(|a| a.transcript)
            .ok_or_else(|| Error::Parse("sem alternativas na resposta".into()))?;
        Ok(Transcript {
            text: text.trim().to_owned(),
            engine: self.engine_id(),
        })
    }
}

impl SttProvider for Deepgram {
    fn engine_id(&self) -> String {
        "cloud:deepgram".into()
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

    #[tokio::test]
    async fn posts_wav_and_reads_first_alternative() -> Result<()> {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/listen")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("model".into(), "nova-3".into()),
                Matcher::UrlEncoded("language".into(), "pt-BR".into()),
            ]))
            .match_header("authorization", "Token dg")
            .match_header("content-type", "audio/wav")
            .with_body(
                r#"{"results":{"channels":[{"alternatives":[{"transcript":"Bom dia.","confidence":0.98}]}]}}"#,
            )
            .create_async()
            .await;
        let stt = Deepgram::new("dg".into()).with_base_url(server.url());
        let rec = Recording {
            samples: vec![0.0; 160],
            sample_rate: 16_000,
        };
        let t = stt.transcribe(&rec, Language::PtBr).await?;
        assert_eq!(t.text, "Bom dia.");
        mock.assert_async().await;
        Ok(())
    }
}
