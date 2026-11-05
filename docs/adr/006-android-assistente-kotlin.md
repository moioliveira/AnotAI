# ADR-006 — Android v1: assistente de voz em Kotlin nativo

- Status: Proposta (2026-11-05)

## Contexto

O spec previa o Android na v2, como teclado (IME) em Kotlin com o núcleo Rust via UniFFI. O pedido mudou: no celular, o AnotAI deve ser também um **assistente que age no aparelho**, chamado por saudação ("Bom dia AnotAI, ative o despertador", "Boa tarde AnotAI, mande uma mensagem…", "Boa noite AnotAI, qual a agenda de amanhã").

## Decisão

- App nativo em **Kotlin + Jetpack Compose** (`apps/android`), minSdk 26, sem o núcleo Rust nesta versão.
- **Reconhecimento e fala do próprio Android** (`SpeechRecognizer` e `TextToSpeech`, pt-BR): grátis, sem chave de API, offline quando o pacote de idioma está no aparelho.
- **Interpretador de comandos por regras** em Kotlin puro (`core/`), coberto por testes JUnit: frase de ativação, horários e números falados, dias, contatos e textos com acentos preservados.
- Ações por intents do sistema (despertador, timer, agenda, discador, WhatsApp) e APIs diretas quando há permissão (SMS, ligação, contatos, calendário, lanterna). SMS e ligação pedem confirmação por voz; o WhatsApp abre com o texto pronto e o envio é um toque do usuário.
- Formas de chamar: botão do app, atalho do ícone, bloco nas Configurações rápidas e **escuta contínua experimental** (serviço em primeiro plano com notificação, que espera "Bom dia/Boa tarde/Boa noite/Oi AnotAI").

## Alternativas

- Tauri Android + núcleo Rust: reaproveita código, mas o assistente depende de APIs Android (intents, serviços, permissões) que ficariam em Kotlin de qualquer jeito; o ganho não paga a complexidade na v1.
- Palavra de ativação dedicada (Porcupine, openWakeWord): mais eficiente que reiniciar o `SpeechRecognizer`, mas exige chave/conta ou modelos próprios. Fica para a v2.
- LLM para interpretar comandos: mais flexível, mas exige rede e chave; as regras cobrem os comandos básicos offline. Um fallback por LLM pode entrar depois.

## Consequências

- A escuta contínua gasta bateria e, em alguns aparelhos, toca um bipe a cada reinício do reconhecedor. Com a tela em outro app, abrir WhatsApp/agenda/discador exige a permissão "sobrepor a outros apps" (restrição do Android a abrir telas a partir de segundo plano).
- Comandos fora das regras recebem "Ainda não sei fazer isso".
- O teclado (IME) e o núcleo Rust via UniFFI continuam no plano para o ditado no celular.
