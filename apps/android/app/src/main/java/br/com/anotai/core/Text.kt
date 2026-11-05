package br.com.anotai.core

import java.text.Normalizer

/**
 * Minúsculas e sem acentos, caractere por caractere: o resultado tem sempre o
 * mesmo tamanho do original, então um trecho achado no texto "dobrado" pode
 * ser recortado do original (com acentos e maiúsculas) pelos mesmos índices.
 */
fun fold(s: String): String = buildString(s.length) {
    for (c in s) {
        val lower = c.lowercaseChar()
        append(Normalizer.normalize(lower.toString(), Normalizer.Form.NFD)[0])
    }
}

private val UNITS = mapOf(
    "zero" to 0, "um" to 1, "uma" to 1, "dois" to 2, "duas" to 2, "tres" to 3,
    "quatro" to 4, "cinco" to 5, "seis" to 6, "sete" to 7, "oito" to 8, "nove" to 9,
)
private val TEENS = mapOf(
    "dez" to 10, "onze" to 11, "doze" to 12, "treze" to 13, "catorze" to 14,
    "quatorze" to 14, "quinze" to 15, "dezesseis" to 16, "dezessete" to 17,
    "dezoito" to 18, "dezenove" to 19,
)
private val TENS = mapOf("vinte" to 20, "trinta" to 30, "quarenta" to 40, "cinquenta" to 50, "sessenta" to 60)

// "um"/"uma" também são artigos ("marque uma reunião"): só viram número em
// contexto de horário ou duração.
private val ARTICLE_LIKE = setOf("um", "uma")
private val NUMBER_WORDS = Regex(
    "\\b(?:(${TENS.keys.joinToString("|")})(?:\\s+e\\s+(${UNITS.keys.joinToString("|")}))?" +
        "|(${(TEENS.keys + UNITS.keys - ARTICLE_LIKE).joinToString("|")}))\\b",
)
private val ONE_AS_NUMBER = Regex(
    "(?<=\\b(?:as|a|pras?|ate as|de)\\s)(uma?)\\b" +
        "|\\b(uma?)(?=\\s+(?:hora|horas|minuto|minutos|segundo|segundos|e\\s+meia|e\\s+\\d|da\\s+(?:manha|tarde|noite|madrugada))\\b)",
)

/**
 * Troca números por extenso por dígitos ("sete e meia" → "7 e meia",
 * "vinte e cinco" → "25"). Recebe texto já dobrado.
 *
 * Mantém o tamanho do texto: os dígitos são completados com espaços (todo
 * número por extenso é mais longo que os dígitos). Assim um trecho achado
 * aqui pode ser recortado do texto original pelos mesmos índices.
 */
fun wordsToDigits(folded: String): String {
    fun pad(digits: String, length: Int) = digits.padEnd(length, ' ')
    val numbers = NUMBER_WORDS.replace(folded) { m ->
        val (tens, unit, single) = m.destructured
        val value = when {
            tens.isNotEmpty() -> TENS.getValue(tens) + (UNITS[unit] ?: 0)
            else -> TEENS[single] ?: UNITS.getValue(single)
        }
        pad(value.toString(), m.value.length)
    }
    return ONE_AS_NUMBER.replace(numbers) { m -> pad("1", m.value.length) }
}

/** Primeira letra maiúscula, o resto como está. */
fun String.capitalizeFirst(): String = replaceFirstChar { it.uppercaseChar() }
