# ADR-003 — STT plugável, provedor escolhido por benchmark

- Status: Proposta (2026-11-05) — decisão final depende do benchmark da Sprint 0

## Contexto

Qualidade (WER em pt-BR), latência e custo variam muito entre provedores e mudam com frequência. Clientes corporativos podem exigir processamento local.

## Decisão

Trait `SttProvider` em `anotai-stt` com adaptadores de nuvem (OpenAI gpt-4o-transcribe, Deepgram Nova-3, Groq Whisper large-v3-turbo) e local (whisper.cpp via `whisper-rs`, large-v3-turbo quantizado). Modo `auto` = nuvem com fallback local. O provedor padrão sai de `tools/benchmark` (WER, latência p50/p95, custo por hora).

## Consequências

- Trocar de fornecedor não mexe no app.
- O benchmark precisa de um conjunto de áudios rotulados em pt-BR e EN, mantido no repositório (ou em armazenamento com acesso controlado, se houver dados sensíveis).
