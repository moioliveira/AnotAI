# AnotAI

Ditado por voz para Windows (depois Linux e Android). Segure **Ctrl + Win** para falar; toque duas vezes para o modo contínuo. O texto entra no cursor já corrigido (norma culta), com módulo de tradução EN→PT.

- PRD, Tech Spec e guia do time: https://claude.ai/artifact/Mj8mNM5pZPEGo5eHkYzpBE (cópia em PDF em `doc/`)
- Identidade visual e mockups: https://claude.ai/artifact/JL6UCmBYVtpBVnixDajMAf

## Pré-requisitos (Windows 11)

- Visual Studio Build Tools com o workload "Desktop development with C++"
- WebView2 (já vem no Windows 11)
- Rust stable via rustup (`x86_64-pc-windows-msvc`); `rust-toolchain.toml` fixa os componentes
- Node.js 24+ com pnpm via Corepack (`corepack enable`, ou `corepack pnpm …` sem permissão de admin)
- Para o motor local (whisper.cpp): **CMake** (vem no VS Build Tools; ponha no PATH ou defina `CMAKE`) e **libclang** para o bindgen (LLVM, ou `pip install --target <pasta> libclang` sem admin). Defina `CMAKE` e `LIBCLANG_PATH` no `[env]` do `~/.cargo/config.toml` se não estiverem no PATH. As flags de CPU e otimização do whisper.cpp já estão em `.cargo/config.toml` (ver ADR-005).

## Comandos

```sh
pnpm install          # dependências JS + git hooks (lefthook)
pnpm dev              # abre o app com hot reload (tauri dev)
pnpm test             # Vitest + cargo test
pnpm lint             # Biome + clippy -D warnings
pnpm format           # Biome + rustfmt
pnpm build:app        # instalador NSIS/MSI em target/release/bundle
```

## Estrutura

```
apps/desktop/          Tauri 2: src/ (React 19 + Tailwind v4) e src-tauri/ (janelas, bandeja, plugins)
apps/mobile/           v2: Tauri Android + IME em Kotlin
packages/ui/           componentes estilo shadcn/ui com a identidade AnotAI
packages/design-tokens/ paleta, fontes e raios (CSS @theme + TS)
packages/i18n/         textos pt-BR
crates/anotai-core/    orquestra o pipeline; eventos para a UI
crates/anotai-hotkey/  hook WH_KEYBOARD_LL + tecla-máscara + máquina de estados Ctrl + Win (com testes)
crates/anotai-audio/   microfone (cpal) → mono → 16 kHz (rubato), nível para o overlay
crates/anotai-platform/ app em primeiro plano e detecção de app elevado (UIPI)
crates/anotai-stt/     transcrição: OpenAI gpt-4o-transcribe, Groq Whisper, Deepgram Nova-3 e whisper.cpp local
crates/anotai-postproc/ comandos de voz, hesitações, dicionário, correção/tradução com Claude Haiku 5.5
crates/anotai-inject/  SendInput (Unicode) ou colar + restaurar; bloqueia campos de senha
crates/anotai-store/   SQLite (sqlx): histórico com busca FTS5, dicionário, estatísticas
crates/anotai-vad/     Silero VAD (whisper.cpp) e segmentação do modo contínuo por pausas
services/api-proxy/    v1.1
tools/benchmark/       WER e latência por motor (Sprint 0) — ver tools/benchmark/README.md
docs/adr/              decisões de arquitetura
```

## Padrões

Conventional Commits (validado no `commit-msg`), TypeScript strict sem `any`, `clippy -D warnings`, sem `unwrap()` fora de testes, cores só pelos tokens de `@anotai/design-tokens`. Detalhes na seção 7.3 do Tech Spec.

## Primeiro uso

`pnpm dev`. Na primeira execução, o **onboarding** pergunta onde a voz é processada:

- **Nuvem** (melhor qualidade): pede o consentimento LGPD e as chaves de API (provedor de transcrição, padrão provisório Groq; e Anthropic, opcional, para corrigir pela norma culta e traduzir). As chaves vão para o Gerenciador de Credenciais do Windows; em debug, `GROQ_API_KEY`, `OPENAI_API_KEY`, `DEEPGRAM_API_KEY` e `ANTHROPIC_API_KEY` no ambiente também valem.
- **Só neste computador**: baixa um modelo whisper.cpp (Rápido 60 MB ou Preciso 190 MB, conferidos por SHA-256). Nada sai da máquina.

Depois, teste do microfone e um resumo dos gestos. Sem consentimento, nenhum áudio ou texto vai para a nuvem; dá para revogar em **Configurações → Privacidade**.

Uso:
- **Segure Ctrl + Win**, fale e solte: o texto aparece no cursor.
- **Toque duas vezes** para o modo contínuo: cada frase é inserida quando você faz uma pausa (~1,2 s); um toque encerra.
- A bandeja (ou a tela Início) alterna entre ditado e **tradução EN→PT**.

O que acontece com cada fala: motor de transcrição (nuvem, local ou automático com fallback local se a rede cair) → regras (comandos de voz como "vírgula", "ponto final", "nova linha"; hesitações; dicionário) → Claude Haiku 5.5 corrige ou traduz (limite de 1,5 s; se passar, entra o texto das regras) → inserção no app de origem → histórico.

- Até 200 caracteres o texto é digitado; acima, colado (o que estava copiado volta depois de 300 ms). Quebra de linha vira Shift+Enter, para não enviar mensagens em apps de chat.
- Campos de senha bloqueiam a inserção. Se a janela de origem não puder ser reativada, o texto fica na área de transferência.
- Gravações curtas (< 0,4 s) ou silenciosas são ignoradas sem chamar o provedor.

## Testes e depuração

- **Configurações → Privacidade → Guardar gravações** (ou `ANOTAI_SAVE_RECORDINGS=1`): cada gravação vira um WAV 16 kHz em `%APPDATA%\AnotAI\recordings`.
- Microfone sem o app: `cargo test -p anotai-audio --test microphone -- --ignored --nocapture`.
- Motor local e VAD sobre WAVs: ver o cabeçalho de `crates/anotai-stt/tests/local_whisper.rs` e `crates/anotai-vad/tests/silero.rs` (rode com `--release`).
- Testes automatizados com `SendInput`: rode em debug com `ANOTAI_HOTKEY_ACCEPT_INJECTED=1`; em produção, teclas injetadas por outros programas são ignoradas.
- Dados: `%APPDATA%\AnotAI\anotai.db` (histórico, dicionário, estatísticas). Configurações: `settings.json` do tauri-plugin-store.

## Ainda não feito

- **Streaming** (transcrição parcial enquanto fala, Deepgram/OpenAI por WebSocket): hoje a transcrição começa ao soltar a tecla ou a cada pausa no modo contínuo.
- **Aceleração por GPU** do motor local (Vulkan) e o modelo large-v3-turbo.
- **Atualizador automático e assinatura de código**: dependem de decisões (repositório de releases, chave do updater, certificado/Azure Trusted Signing).
- **Benchmark com dataset real** (vozes do time) para escolher o provedor padrão; tauri-specta.
