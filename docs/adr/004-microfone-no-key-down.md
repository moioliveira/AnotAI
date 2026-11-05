# ADR-004 — Microfone abre no key-down, sem buffer contínuo

- Status: Proposta (2026-11-05)

## Contexto

O Tech Spec (5.2) pede um pré-buffer de 300 ms para não perder a primeira sílaba. A forma literal seria manter o microfone sempre aberto num buffer circular. Isso deixa o indicador de microfone do Windows aceso o tempo todo, assusta o usuário e complica a LGPD (áudio sendo capturado sem ditado em curso). Além disso, o gesto "segurar" só é reconhecido 250 ms depois do key-down.

## Decisão

A máquina de gestos emite `Arm` no instante em que Ctrl + Win (ou o atalho alternativo) é pressionado e `Disarm` quando o gesto vira um toque isolado ou um atalho do Windows (ex.: Ctrl + Win + ←). O `anotai-audio` abre o microfone no `Arm` e descarta o áudio no `Disarm`. Assim, o áudio desde o key-down é preservado sem microfone ligado em repouso.

## Alternativas

- Buffer circular contínuo (texto do spec): menor latência de abertura, mas microfone sempre ligado.
- Abrir só no `StartPushToTalk`: perde ~250 ms + o tempo de abertura do dispositivo.

## Consequências

- O microfone liga por instantes também quando o usuário usa Ctrl + Win + setas para trocar de área de trabalho; o áudio é descartado e nunca sai da memória.
- O tempo de abertura do WASAPI (dezenas de ms) ainda pode cortar o início de uma fala muito rápida. Medir na Sprint 0; se for problema, manter o dispositivo aberto por alguns segundos após o último ditado.
