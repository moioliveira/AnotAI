package br.com.anotai.core

import java.time.LocalDate
import java.time.LocalTime

enum class Channel { WHATSAPP, SMS }

/** O que o usuário pediu. Campos `null` são perguntados em seguida. */
sealed interface Command {
    data class SetAlarm(val time: LocalTime?, val label: String? = null) : Command
    data class SetTimer(val seconds: Long?) : Command
    data class SendMessage(val contact: String?, val text: String?, val channel: Channel?) : Command
    data class Call(val contact: String?) : Command
    data class Agenda(val date: LocalDate) : Command
    data class CreateEvent(val title: String?, val date: LocalDate?, val time: LocalTime?) : Command
    data class Note(val text: String?) : Command
    data class Flashlight(val on: Boolean) : Command
    data class OpenApp(val name: String) : Command
    data object TellTime : Command
    data object TellDate : Command
    data object Help : Command
    data class Unknown(val text: String) : Command
}

/** Respostas de sim/não em confirmações. */
object Confirm {
    private val YES = Regex("\\b(sim|pode|confirma\\w*|manda|envia|isso|claro|ok|certo|liga|positivo)\\b")
    private val NO = Regex("\\b(nao|cancela\\w*|para|deixa|esquece|negativo)\\b")

    fun parse(text: String): Boolean? {
        val t = fold(text)
        return when {
            NO.containsMatchIn(t) -> false
            YES.containsMatchIn(t) -> true
            else -> null
        }
    }

    fun isCancel(text: String): Boolean =
        Regex("\\b(cancela\\w*|deixa\\s+(?:pra|para)\\s+la|esquece|para\\s+tudo)\\b").containsMatchIn(fold(text))
}
