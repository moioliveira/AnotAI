package br.com.anotai.core

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalTime

/** Interpreta horários, durações e dias falados em português. */
object TimeParser {
    private val NOON = Regex("\\bmeio[ -]?dia(?:\\s+e\\s+(meia|\\d{1,2}))?\\b")
    private val MIDNIGHT = Regex("\\bmeia[ -]?noite(?:\\s+e\\s+(meia|\\d{1,2}))?\\b")
    private val HHMM = Regex("\\b(\\d{1,2})\\s*(?::|h)\\s*(\\d{2})\\b")
    private val HOUR = Regex(
        "\\b(\\d{1,2})(?:\\s*(?:h|horas?)\\b)?(?:\\s+e\\s+(meia|\\d{1,2})(?:\\s*minutos?)?\\b)?",
    )
    private val NOT_A_CLOCK = Regex("^\\s*(?:minutos?|min|segundos?|dias?|semanas?|vezes)\\b")
    private val RELATIVE = Regex("\\b(?:daqui\\s+a|daqui|em|dentro\\s+de)\\s+(?:\\d|meia|uma)")

    /** Texto pronto para os padrões: dobrado e com números em dígitos. */
    private fun prep(raw: String) = wordsToDigits(fold(raw))

    private fun minutesOf(s: String): Int = if (s == "meia") 30 else s.toIntOrNull() ?: 0

    /** "às 7 e meia da noite" → 19:30. `null` se não há horário. */
    fun clock(raw: String): LocalTime? {
        val t = prep(raw)
        if (RELATIVE.containsMatchIn(t)) return null
        NOON.find(t)?.let { return LocalTime.of(12, minutesOf(it.groupValues[1])) }
        MIDNIGHT.find(t)?.let { return LocalTime.of(0, minutesOf(it.groupValues[1])) }

        var hour: Int? = null
        var minute = 0
        var end = 0
        val hhmm = HHMM.find(t)
        if (hhmm != null) {
            hour = hhmm.groupValues[1].toInt()
            minute = hhmm.groupValues[2].toInt()
            end = hhmm.range.last + 1
        } else {
            for (m in HOUR.findAll(t)) {
                if (NOT_A_CLOCK.containsMatchIn(t.substring(m.range.last + 1))) continue
                hour = m.groupValues[1].toInt()
                minute = minutesOf(m.groupValues[2])
                end = m.range.last + 1
                break
            }
        }
        if (hour == null || hour > 23 || minute > 59) return null

        val after = t.substring(end)
        when {
            Regex("^\\s*da\\s+tarde").containsMatchIn(after) && hour in 1..11 -> hour += 12
            Regex("^\\s*da\\s+noite").containsMatchIn(after) && hour in 5..11 -> hour += 12
            Regex("^\\s*da\\s+noite").containsMatchIn(after) && hour == 12 -> hour = 0
        }
        return LocalTime.of(hour, minute)
    }

    /** "daqui a 1 hora e meia" → 5400 s. `null` se não há duração. */
    fun durationSeconds(raw: String): Long? {
        val t = prep(raw)
        var total = 0L
        var found = false
        Regex("\\b(\\d+)\\s*horas?\\s+e\\s+meia\\b").find(t)?.let {
            return it.groupValues[1].toLong() * 3600 + 1800
        }
        if (Regex("\\bmeia\\s+hora\\b").containsMatchIn(t)) {
            total += 1800
            found = true
        }
        for ((pattern, unit) in listOf(
            Regex("\\b(\\d+)\\s*(?:horas?|h)\\b") to 3600L,
            Regex("\\b(\\d+)\\s*(?:minutos?|min)\\b") to 60L,
            Regex("\\b(\\d+)\\s*(?:segundos?|seg|s)\\b") to 1L,
        )) {
            pattern.find(t)?.let {
                total += it.groupValues[1].toLong() * unit
                found = true
            }
        }
        return if (found && total > 0) total else null
    }

    private val WEEKDAYS = mapOf(
        "segunda" to DayOfWeek.MONDAY, "terca" to DayOfWeek.TUESDAY,
        "quarta" to DayOfWeek.WEDNESDAY, "quinta" to DayOfWeek.THURSDAY,
        "sexta" to DayOfWeek.FRIDAY, "sabado" to DayOfWeek.SATURDAY,
        "domingo" to DayOfWeek.SUNDAY,
    )

    /** "amanhã", "depois de amanhã", "sexta", "dia 15". `null` se não há dia. */
    fun day(raw: String, today: LocalDate): LocalDate? {
        val t = prep(raw)
        if (Regex("\\bdepois\\s+de\\s+amanha\\b").containsMatchIn(t)) return today.plusDays(2)
        if (Regex("\\bamanha\\b").containsMatchIn(t)) return today.plusDays(1)
        if (Regex("\\bhoje\\b").containsMatchIn(t)) return today
        for ((name, dow) in WEEKDAYS) {
            if (Regex("\\b$name(?:-feira| feira)?\\b").containsMatchIn(t)) {
                val delta = (dow.value - today.dayOfWeek.value + 7) % 7
                return today.plusDays(delta.toLong())
            }
        }
        Regex("\\bdia\\s+(\\d{1,2})\\b").find(t)?.let {
            val d = it.groupValues[1].toInt()
            if (d !in 1..31) return null
            val thisMonth = runCatching { today.withDayOfMonth(d) }.getOrNull()
            return if (thisMonth != null && !thisMonth.isBefore(today)) {
                thisMonth
            } else {
                runCatching { today.plusMonths(1).withDayOfMonth(d) }.getOrNull()
            }
        }
        return null
    }
}
