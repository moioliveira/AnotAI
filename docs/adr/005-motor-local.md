# ADR-005 — Motor local com whisper.cpp na CPU

- Status: Proposta (2026-11-05)

## Contexto

O modo `auto` precisa de fallback offline, e clientes com dados sensíveis querem que o áudio não saia do computador. A máquina de referência (Intel i7-1365U, Iris Xe, sem Vulkan) só tem CPU.

## Decisão

- `whisper-rs` 0.16 (whisper.cpp) no `anotai-stt`, com o VAD Silero embutido do próprio whisper.cpp (o modelo de 885 KB vai no instalador).
- Modelos baixados sob demanda, conferidos por SHA-256: **base** (60 MB, padrão, ~2,7 s por frase, WER 8,2% em voz sintética) e **small** (190 MB, ~9,6 s, WER 4,5%). O large-v3-turbo do Tech Spec fica para quando houver aceleração por GPU (feature `vulkan`).
- Build: `GGML_AVX2/FMA/F16C` fixos e `CL=/O2` em `.cargo/config.toml`. Com MSVC, o crate `cmake` descartava as flags de otimização e o whisper.cpp saía ~10x mais lento (tiny: 10,6 s → 1,1 s).
- Filtro de alucinações comuns do Whisper em silêncio ("Legendas pela comunidade Amara.org", "Obrigado por assistir").

## Consequências

- Compilar exige CMake (vem no VS Build Tools) e libclang para o bindgen; ver README.
- Na CPU, o local é fallback e opção de privacidade, não o caminho principal: a nuvem continua mais rápida e precisa.
- O mesmo `LocalWhisper` serve o benchmark (`tools/benchmark`).
