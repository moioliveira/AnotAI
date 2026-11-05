# ADR-001 — Tauri 2 + núcleo Rust + React

- Status: Proposta (2026-11-05)

## Contexto

O app precisa de hook de teclado global, captura de áudio, UI Automation e SendInput no Windows, e depois rodar no Linux e no Android reaproveitando o máximo de código. Deve ficar leve na bandeja.

## Decisão

Tauri 2 como shell, núcleo em Rust (`crates/anotai-*`) com acesso direto ao Win32 via `windows-rs`, e UI em React 19 + TypeScript + Tailwind v4 + shadcn/ui. A UI só mostra estado e configurações; o ditado roda no núcleo mesmo com a janela fechada.

## Alternativas

- Electron: instalador ~100 MB e mais RAM.
- .NET MAUI / WinUI 3: não cobre Linux.
- Flutter: acesso nativo ao Win32 mais trabalhoso.

## Consequências

- Instalador ~10 MB; o mesmo núcleo serve Linux e Android (UniFFI no IME Kotlin).
- O time precisa de experiência em Rust para hook, áudio e inserção.
