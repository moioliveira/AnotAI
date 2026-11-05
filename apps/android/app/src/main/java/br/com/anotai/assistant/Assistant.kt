package br.com.anotai.assistant

import android.Manifest
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import br.com.anotai.actions.DeviceActions
import br.com.anotai.actions.NoteStore
import br.com.anotai.core.Channel
import br.com.anotai.core.Command
import br.com.anotai.core.CommandParser
import br.com.anotai.core.Confirm
import br.com.anotai.core.Greeting
import br.com.anotai.core.Phone
import br.com.anotai.core.Speech
import br.com.anotai.core.TimeParser
import br.com.anotai.core.WakePhrase
import br.com.anotai.core.capitalizeFirst
import br.com.anotai.speech.VoiceInput
import br.com.anotai.speech.VoiceOutput
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * O assistente: ouve, entende, age e responde por voz. Uma instância por
 * processo (ver [br.com.anotai.AnotAIApp]), usada pela tela e pela escuta
 * contínua. Tudo roda na thread principal.
 */
class Assistant(context: Context) {
    enum class Status { IDLE, LISTENING, THINKING, SPEAKING }

    data class UiState(
        val status: Status = Status.IDLE,
        val heard: String = "",
        val reply: String = "",
        val level: Float = 0f,
        val continuous: Boolean = false,
        /** Permissão que faltou para o último pedido (a tela pede ao usuário). */
        val missingPermission: String? = null,
        val notesVersion: Int = 0,
    )

    private val app = context.applicationContext
    private val main = Handler(Looper.getMainLooper())
    private val input = VoiceInput(app)
    private val output = VoiceOutput(app)
    val actions = DeviceActions(app)
    val notes = NoteStore(app)

    private val _ui = MutableStateFlow(UiState())
    val ui: StateFlow<UiState> = _ui.asStateFlow()

    /** Pergunta em aberto: a próxima frase responde a ela. */
    private sealed interface Pending {
        data class AlarmTime(val label: String?) : Pending
        data object TimerLength : Pending
        data class MessageContact(val text: String?, val channel: Channel?) : Pending
        data class MessageText(val contact: DeviceActions.Contact, val channel: Channel?) : Pending
        data class SmsConfirm(val contact: DeviceActions.Contact, val text: String) : Pending
        data object CallContact : Pending
        data class CallConfirm(val contact: DeviceActions.Contact) : Pending
        data class EventTitle(val date: LocalDate?, val time: LocalTime?) : Pending
        data object NoteText : Pending
        data object AnyCommand : Pending
    }

    private var pending: Pending? = null
    private var retries = 0
    private var requireWake = false
    private var greeting: Greeting? = null

    // ---------------------------------------------------------------- escuta

    /** Escuta um pedido (botão, atalho, bloco). */
    fun listen() {
        requireWake = false
        startListening()
    }

    /** Liga/desliga a escuta contínua, que espera "Bom dia AnotAI…". */
    fun setContinuous(on: Boolean) {
        _ui.update { it.copy(continuous = on) }
        if (on && _ui.value.status == Status.IDLE) {
            requireWake = true
            startListening()
        } else if (!on && pending == null) {
            input.destroy()
            setStatus(Status.IDLE)
        }
    }

    fun stopListening() = input.stop()

    fun clearMissingPermission() = _ui.update { it.copy(missingPermission = null) }

    fun shutdown() {
        input.destroy()
        output.shutdown()
    }

    private fun setStatus(s: Status) = _ui.update { it.copy(status = s, level = 0f) }

    private fun startListening() {
        setStatus(Status.LISTENING)
        input.listen(
            onPartial = { text -> if (!requireWake) _ui.update { it.copy(heard = text) } },
            onLevel = { db -> _ui.update { it.copy(level = ((db + 2f) / 12f).coerceIn(0f, 1f)) } },
            onDone = { text, error -> onHeard(text, error) },
        )
    }

    /** Volta a ouvir sozinho na escuta contínua; fica parado no modo manual. */
    private fun idle() {
        setStatus(Status.IDLE)
        if (_ui.value.continuous) {
            requireWake = true
            main.postDelayed({
                if (_ui.value.continuous && _ui.value.status == Status.IDLE) startListening()
            }, RELISTEN_DELAY_MS)
        }
    }

    private fun onHeard(text: String?, error: Int?) {
        if (text.isNullOrBlank()) {
            // Silêncio durante uma pergunta: repete uma vez.
            if (pending != null && VoiceInput.isSilence(error) && retries < 1) {
                retries++
                say("Não ouvi. Pode repetir?") { startListening() }
                return
            }
            if (pending != null) pending = null
            // Erro de verdade (não silêncio) na escuta contínua: espera mais.
            if (_ui.value.continuous && error != null && !VoiceInput.isSilence(error)) {
                setStatus(Status.IDLE)
                main.postDelayed({ idle() }, ERROR_BACKOFF_MS)
            } else {
                idle()
            }
            return
        }
        handle(text)
    }

    // --------------------------------------------------------------- pedidos

    /** Pedido digitado na tela: não exige a frase de ativação. */
    fun handleTyped(text: String) {
        requireWake = false
        input.destroy()
        handle(text)
    }

    /** Interpreta uma frase ouvida. */
    private fun handle(text: String) {
        Log.i(TAG, "ouvido: $text")
        setStatus(Status.THINKING)
        val p = pending
        if (p != null) {
            _ui.update { it.copy(heard = text) }
            if (Confirm.isCancel(text)) {
                pending = null
                say("Tudo bem, cancelei.") { idle() }
            } else {
                answer(p, text)
            }
            return
        }

        val today = LocalDate.now()
        val wake = WakePhrase.find(text)
        val command: Command
        when {
            wake == null && requireWake -> {
                idle()
                return
            }
            wake == null -> {
                greeting = null
                command = CommandParser.parse(text, today)
            }
            wake.greeting == null && !requireWake -> {
                // "anota aí comprar pão" é um comando, não o nome do app.
                val whole = CommandParser.parse(text, today)
                val stripped = CommandParser.parse(wake.command, today)
                command = if (whole !is Command.Unknown && stripped is Command.Unknown) whole else stripped
                greeting = null
            }
            else -> {
                greeting = wake.greeting
                if (wake.command.isBlank()) {
                    _ui.update { it.copy(heard = text) }
                    ask("${greeting?.reply ?: "Oi!"} Pode falar.", Pending.AnyCommand)
                    return
                }
                command = CommandParser.parse(wake.command, today)
            }
        }
        _ui.update { it.copy(heard = text) }
        execute(command)
    }

    private fun answer(p: Pending, text: String) {
        val today = LocalDate.now()
        pending = null
        when (p) {
            is Pending.AnyCommand -> execute(CommandParser.parse(text, today))
            is Pending.AlarmTime -> {
                val time = TimeParser.clock(text)
                    ?: TimeParser.durationSeconds(text)?.let { LocalTime.now().plusSeconds(it) }
                if (time == null) retry("Não entendi o horário. Para que horas?", p) else execute(Command.SetAlarm(time, p.label))
            }
            is Pending.TimerLength -> {
                val s = TimeParser.durationSeconds(text)
                if (s == null) retry("Não entendi. Por quanto tempo?", p) else execute(Command.SetTimer(s))
            }
            is Pending.MessageContact -> execute(Command.SendMessage(text, p.text, p.channel))
            is Pending.MessageText -> sendMessage(p.contact, text.capitalizeFirst(), p.channel)
            is Pending.SmsConfirm -> when (Confirm.parse(text)) {
                true -> say(
                    if (actions.sendSms(p.contact.phone, p.text)) "Mensagem enviada para ${p.contact.name}."
                    else "Não consegui enviar o SMS.",
                ) { idle() }
                false -> say("Tudo bem, não enviei.") { idle() }
                null -> retry("Diga sim para enviar ou não para cancelar.", p)
            }
            is Pending.CallContact -> execute(Command.Call(text))
            is Pending.CallConfirm -> when (Confirm.parse(text)) {
                true -> {
                    actions.call(p.contact.phone)
                    say("Ligando para ${p.contact.name}.") { idle() }
                }
                false -> say("Tudo bem.") { idle() }
                null -> retry("Diga sim para ligar ou não para cancelar.", p)
            }
            is Pending.EventTitle -> execute(Command.CreateEvent(text.capitalizeFirst(), p.date, p.time))
            is Pending.NoteText -> execute(Command.Note(text))
        }
    }

    private fun retry(question: String, p: Pending) {
        if (retries >= 2) {
            retries = 0
            say("Vamos deixar para depois.") { idle() }
        } else {
            retries++
            ask(question, p)
        }
    }

    private fun execute(command: Command) {
        retries = 0
        val today = LocalDate.now()
        when (command) {
            is Command.SetAlarm -> {
                val time = command.time ?: return ask("Para que horas?", Pending.AlarmTime(command.label))
                val ok = actions.setAlarm(time, command.label)
                reply(
                    if (ok) "Despertador marcado ${Speech.forTime(time)}" +
                        (command.label?.let { ", com o lembrete: $it." } ?: ".")
                    else "Não encontrei um app de despertador.",
                )
            }
            is Command.SetTimer -> {
                val s = command.seconds ?: return ask("Por quanto tempo?", Pending.TimerLength)
                reply(if (actions.setTimer(s)) "Timer de ${Speech.duration(s)} iniciado." else "Não consegui iniciar o timer.")
            }
            is Command.SendMessage -> {
                val name = command.contact
                    ?: return ask("Para quem é a mensagem?", Pending.MessageContact(command.text, command.channel))
                if (!need(Manifest.permission.READ_CONTACTS, "ler seus contatos")) return
                val contact = actions.findContact(name)
                    ?: return retry("Não achei $name nos contatos. Para quem é a mensagem?", Pending.MessageContact(command.text, command.channel))
                val text = command.text
                    ?: return ask("Qual a mensagem para ${contact.name}?", Pending.MessageText(contact, command.channel))
                sendMessage(contact, text, command.channel)
            }
            is Command.Call -> {
                val name = command.contact ?: return ask("Para quem devo ligar?", Pending.CallContact)
                if (!need(Manifest.permission.READ_CONTACTS, "ler seus contatos")) return
                val contact = actions.findContact(name)
                    ?: return retry("Não achei $name nos contatos. Para quem devo ligar?", Pending.CallContact)
                ask("Ligar para ${contact.name}?", Pending.CallConfirm(contact))
            }
            is Command.Agenda -> {
                if (!need(Manifest.permission.READ_CALENDAR, "ler sua agenda")) return
                val items = actions.agenda(command.date) ?: emptyList()
                reply(Speech.agenda(items, command.date, today))
            }
            is Command.CreateEvent -> {
                val title = command.title
                    ?: return ask("Qual o nome do evento?", Pending.EventTitle(command.date, command.time))
                val date = command.date ?: today
                val ok = actions.createEvent(title, date, command.time)
                val whenText = Speech.day(date, today) + (command.time?.let { " " + Speech.atTime(it) } ?: "")
                reply(if (ok) "Abri a agenda com $title, $whenText. Confira e salve." else "Não encontrei um app de agenda.")
            }
            is Command.Note -> {
                val text = command.text ?: return ask("O que devo anotar?", Pending.NoteText)
                notes.add(text.capitalizeFirst())
                _ui.update { it.copy(notesVersion = it.notesVersion + 1) }
                reply("Anotado: $text.")
            }
            is Command.Flashlight -> reply(
                when {
                    !actions.torch(command.on) -> "Não consegui controlar a lanterna."
                    command.on -> "Lanterna acesa."
                    else -> "Lanterna apagada."
                },
            )
            is Command.OpenApp -> {
                val opened = actions.openApp(command.name)
                reply(if (opened != null) "Abrindo $opened." else "Não encontrei o app ${command.name}.")
            }
            Command.TellTime -> reply("Agora são ${Speech.time(LocalTime.now())}.")
            Command.TellDate -> reply(
                "Hoje é " + LocalDate.now().format(DateTimeFormatter.ofPattern("EEEE, d 'de' MMMM", PT)) + ".",
            )
            Command.Help -> reply(Speech.HELP)
            is Command.Unknown -> reply(
                "Ainda não sei fazer isso. Diga \"o que você sabe fazer\" para ouvir os comandos.",
            )
        }
    }

    private fun sendMessage(contact: DeviceActions.Contact, text: String, channel: Channel?) {
        val useWhatsApp = channel == Channel.WHATSAPP || (channel == null && actions.hasWhatsApp())
        if (useWhatsApp) {
            val number = Phone.forWhatsApp(contact.phone)
                ?: return reply("O número de ${contact.name} não parece válido para o WhatsApp.")
            reply(
                if (actions.openWhatsApp(number, text)) "Abri o WhatsApp com a mensagem para ${contact.name}. É só tocar em enviar."
                else "Não consegui abrir o WhatsApp.",
            )
        } else {
            if (!need(Manifest.permission.SEND_SMS, "enviar SMS")) return
            ask("Enviar SMS para ${contact.name}: $text. Confirma?", Pending.SmsConfirm(contact, text))
        }
    }

    // -------------------------------------------------------------- respostas

    /** Responde (com a saudação, se o pedido veio com uma) e volta ao repouso. */
    private fun reply(text: String) {
        val prefix = greeting?.reply?.let { "$it " } ?: ""
        greeting = null
        say(prefix + text) { idle() }
    }

    /** Pergunta e já abre o microfone para a resposta. */
    private fun ask(question: String, p: Pending) {
        pending = p
        requireWake = false
        val prefix = greeting?.reply?.takeUnless { question.startsWith(it) }?.let { "$it " } ?: ""
        greeting = null
        say(prefix + question) { startListening() }
    }

    private fun say(text: String, then: () -> Unit) {
        Log.i(TAG, "resposta: $text")
        _ui.update { it.copy(reply = text) }
        setStatus(Status.SPEAKING)
        output.speak(text, then)
    }

    /** Confere uma permissão; sem ela, explica e avisa a tela. */
    private fun need(permission: String, what: String): Boolean {
        if (actions.has(permission)) return true
        _ui.update { it.copy(missingPermission = permission) }
        reply("Preciso de permissão para $what. Abra o AnotAI e toque em Permitir.")
        return false
    }

    private companion object {
        const val TAG = "AnotAI"
        val PT: Locale = Locale.forLanguageTag("pt-BR")
        const val RELISTEN_DELAY_MS = 400L
        const val ERROR_BACKOFF_MS = 3_000L
    }
}
