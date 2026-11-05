# AnotAI para Android

Assistente de voz que age no celular. Chame com uma saudação e faça o pedido:

| Você diz                                                                                     | O AnotAI faz                                                                           |
| -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| "Bom dia AnotAI, me acorde às 6 e meia"                                                      | cria o despertador no Relógio                                                          |
| "Boa tarde AnotAI, mande uma mensagem para a Maria"                                          | pergunta o texto e abre o WhatsApp com a mensagem pronta (ou envia SMS após confirmar) |
| "Boa noite AnotAI, qual a agenda de amanhã?"                                                 | lê os compromissos em voz alta                                                         |
| "Oi AnotAI, ligue para a mãe"                                                                | confirma e liga                                                                        |
| "Oi AnotAI, marque reunião com o João sexta às 10"                                           | abre a agenda com o evento preenchido                                                  |
| "Oi AnotAI, me lembre de tomar o remédio às 8 da noite"                                      | despertador às 20h com o lembrete                                                      |
| "Oi AnotAI, anota aí comprar pão"                                                            | guarda a nota no app                                                                   |
| "Oi AnotAI, timer de 10 minutos" / "acenda a lanterna" / "abra o Spotify" / "que horas são?" | faz na hora                                                                            |

Se faltar algo ("para quem?", "qual a mensagem?", "para que horas?"), ele pergunta. "Cancela" desiste.

## Como chamar

- Botão do app, ou toque longo no ícone → **Falar**.
- **Bloco nas Configurações rápidas**: puxe a cortina, toque no lápis e arraste o bloco "AnotAI".
- **Escuta contínua (experimental)**: ligue na tela inicial do app. Fica ouvindo "Bom dia/Boa tarde/Boa noite/Oi AnotAI" com uma notificação fixa. Gasta mais bateria e, em alguns celulares, toca um bipe a cada reinício da escuta. Para abrir WhatsApp, agenda ou discador com o celular em outro app, permita "Sobrepor a outros apps" quando o AnotAI pedir.

O reconhecimento de voz é o do próprio Android (Google), em pt-BR. Em Configurações → Sistema → Idiomas → Reconhecimento de voz no dispositivo, baixe o português para funcionar sem internet.

## Instalar no celular

1. Copie o `app-release.apk` para o celular (cabo, Drive, WhatsApp para você mesmo…) e abra. Permita "instalar apps desconhecidos" para o app que abriu o arquivo.
2. Ou, com depuração USB ligada: `adb install -r app-release.apk`.
3. Na primeira abertura, toque em **Permitir** (microfone, contatos, agenda, SMS, ligações, notificações).

## Desenvolvimento

Requisitos: JDK 17+ (o Gradle não roda com o JDK 11), Android SDK com a plataforma 36.

```sh
cd apps/android
./gradlew :app:testDebugUnitTest   # testes do interpretador (JVM, sem celular)
./gradlew :app:assembleDebug       # app/build/outputs/apk/debug/app-debug.apk
./gradlew :app:assembleRelease     # assinado com a chave de debug (só para testes)
./gradlew :app:lintDebug
```

`local.properties` (fora do git) aponta o SDK: `sdk.dir=C\:/caminho/android-sdk`.

Testes automáticos no emulador: em builds de **debug**, o app aceita um comando em texto e registra as respostas no logcat (tag `AnotAI`):

```sh
adb shell "am start -a br.com.anotai.LISTEN -n br.com.anotai/.ui.MainActivity --es text 'Bom dia AnotAI, que horas são?'"
adb logcat -s AnotAI:I
```

Estrutura:

```
core/        interpretador em Kotlin puro (frase de ativação, horários, comandos, frases faladas) + testes JUnit
assistant/   diálogo: entende, pergunta o que falta, confirma, age e responde
actions/     ações no aparelho (alarme, timer, contatos, WhatsApp/SMS, ligação, agenda, lanterna, apps, notas)
speech/      SpeechRecognizer e TextToSpeech
service/     escuta contínua (serviço em primeiro plano) e bloco das Configurações rápidas
ui/          tela em Compose, com a identidade AnotAI
```

Decisão de arquitetura: [ADR-006](../../docs/adr/006-android-assistente-kotlin.md).
