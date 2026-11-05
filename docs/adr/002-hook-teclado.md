# ADR-002 — Hook WH_KEYBOARD_LL com tecla-máscara

- Status: Proposta (2026-11-05)

## Contexto

`RegisterHotKey` não aceita combinação só de modificadores (Ctrl + Win). Ao soltar Win, o Windows abre o menu Iniciar. Ctrl + Win + ←/→/D/F4 são atalhos de áreas de trabalho virtuais.

## Decisão

Hook de baixo nível `SetWindowsHookExW(WH_KEYBOARD_LL)` numa thread dedicada com loop de mensagens. Antes do key-up da Win, injetar a tecla neutra VK 0xE8 (técnica do AutoHotkey) para não abrir o Iniciar. O callback responde em < 1 ms e só envia eventos por canal; a lógica de segurar/toque duplo fica na máquina de estados pura `anotai_hotkey::gesture` (testada sem SO).

## Alternativas

- `RegisterHotKey` com outro atalho: perde o gesto de produto.
- Raw Input: não permite suprimir o Iniciar.

## Consequências

- Antivírus podem tratar o hook como keylogger: só Ctrl, Win e teclas pressionadas junto com eles são observadas; nada é registrado. Precisa constar na política de privacidade e ser auditado em code review.
- Apps elevados (UIPI) ficam fora de alcance; mostrar aviso.
- Atalho alternativo configurável (padrão Ctrl + Alt + Espaço) via `RegisterHotKey`, que não depende do hook: serve a teclados sem Win e a ambientes que bloqueiam hooks. Como `WM_HOTKEY` só avisa o key-down, o key-up é detectado consultando a tecla a cada 10 ms. Se outro programa já usa o atalho, o registro falha e só o Ctrl + Win fica ativo (aviso no log).
- Antes de ditar, o app confere se o app em primeiro plano roda como administrador e avisa por notificação.
