package br.com.anotai.core

import java.time.LocalDate
import java.time.LocalTime
import java.time.format.TextStyle
import java.util.Locale

/** Frases faladas pelo assistente (sem Android: testáveis na JVM). */
object Speech {
    private val PT = Locale.forLanguageTag("pt-BR")

    /** 7:00 → "7 horas"; 14:30 → "14 e 30"; 12:00 → "meio-dia". */
    fun time(t: LocalTime): String = when {
        t.hour == 12 && t.minute == 0 -> "meio-dia"
        t.hour == 0 && t.minute == 0 -> "meia-noite"
        t.minute == 0 -> if (t.hour == 1) "1 hora" else "${t.hour} horas"
        t.minute == 30 -> "${t.hour} e meia"
        else -> "${t.hour} e ${t.minute}"
    }

    /** Com a preposição certa: "às 7 horas", "ao meio-dia", "à meia-noite", "à uma hora". */
    fun atTime(t: LocalTime): String = when {
        t.hour == 12 && t.minute == 0 -> "ao meio-dia"
        t.hour == 0 && t.minute == 0 -> "à meia-noite"
        t.hour == 1 -> "à " + time(t).replaceFirst("1", "uma")
        else -> "às ${time(t)}"
    }

    /** "para as 7 horas", "para o meio-dia", "para a meia-noite", "para a uma hora". */
    fun forTime(t: LocalTime): String = when {
        t.hour == 12 && t.minute == 0 -> "para o meio-dia"
        t.hour == 0 && t.minute == 0 -> "para a meia-noite"
        t.hour == 1 -> "para a " + time(t).replaceFirst("1", "uma")
        else -> "para as ${time(t)}"
    }

    fun duration(seconds: Long): String {
        val h = seconds / 3600
        val m = (seconds % 3600) / 60
        val s = seconds % 60
        val parts = buildList {
            if (h > 0) add(if (h == 1L) "1 hora" else "$h horas")
            if (m > 0) add(if (m == 1L) "1 minuto" else "$m minutos")
            if (s > 0) add(if (s == 1L) "1 segundo" else "$s segundos")
        }
        return parts.joinToString(" e ")
    }

    /** "amanhã", "hoje", "sexta-feira, dia 14". */
    fun day(date: LocalDate, today: LocalDate): String = when (date) {
        today -> "hoje"
        today.plusDays(1) -> "amanhã"
        today.plusDays(2) -> "depois de amanhã"
        else -> "${date.dayOfWeek.getDisplayName(TextStyle.FULL, PT)}, dia ${date.dayOfMonth}"
    }

    data class AgendaItem(val title: String, val start: LocalTime?, val allDay: Boolean)

    fun agenda(items: List<AgendaItem>, date: LocalDate, today: LocalDate): String {
        val whenText = day(date, today)
        if (items.isEmpty()) return "Você não tem compromissos ${if (date == today || date == today.plusDays(1)) whenText else "na $whenText"}."
        val prefix = when (date) {
            today, today.plusDays(1), today.plusDays(2) -> whenText.capitalizeFirst()
            else -> "Na $whenText"
        }
        val count = if (items.size == 1) "1 compromisso" else "${items.size} compromissos"
        val list = items.joinToString("; ") { item ->
            when {
                item.allDay || item.start == null -> "${item.title}, o dia todo"
                else -> "${atTime(item.start)}, ${item.title}"
            }
        }
        return "$prefix você tem $count: $list."
    }

    val HELP = "Você pode dizer, por exemplo: bom dia AnotAI, me acorde às 7; " +
        "boa tarde AnotAI, mande uma mensagem para a Maria; " +
        "boa noite AnotAI, qual a minha agenda de amanhã. " +
        "Também ligo para contatos, crio eventos e lembretes, anoto, acendo a lanterna e abro apps."
}
