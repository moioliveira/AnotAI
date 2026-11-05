package br.com.anotai.core

/** Saudação usada para chamar o AnotAI (vira a saudação da resposta). */
enum class Greeting(val reply: String) {
    MORNING("Bom dia!"),
    AFTERNOON("Boa tarde!"),
    NIGHT("Boa noite!"),
    NEUTRAL("Oi!"),
}

/** "Bom dia AnotAI, ative o despertador" → saudação + o comando que vem depois. */
data class Wake(val greeting: Greeting?, val command: String)

object WakePhrase {
    private const val GREETING =
        "(bom dia|boa tarde|boa noite|ola|oi|e ai|ei|ok|hey|alo)"

    // O reconhecedor escreve "AnotAI" de vários jeitos.
    private const val NAME =
        "(?:anot\\s*a\\s*i|anota\\s*a?i|a\\s+nota\\s+a?i|anotar\\s+a?i|anota\\s+ai|anotai)"

    private val PATTERN = Regex("(?:\\b$GREETING[\\s,!.]+)?\\b$NAME\\b[\\s,!.:;-]*")

    /**
     * Procura a frase de ativação em qualquer ponto do texto (na escuta
     * contínua chega ruído antes). Devolve `null` se o nome não aparece.
     */
    fun find(text: String): Wake? {
        val m = PATTERN.find(fold(text)) ?: return null
        val greeting = when (m.groupValues[1]) {
            "bom dia" -> Greeting.MORNING
            "boa tarde" -> Greeting.AFTERNOON
            "boa noite" -> Greeting.NIGHT
            "" -> null
            else -> Greeting.NEUTRAL
        }
        return Wake(greeting, text.substring(m.range.last + 1).trim())
    }
}
