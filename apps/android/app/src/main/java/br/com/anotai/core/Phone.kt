package br.com.anotai.core

/** Números de telefone. */
object Phone {
    /**
     * Formato internacional só com dígitos, como o WhatsApp exige
     * ("(11) 98765-4321" → "5511987654321"). Assume o Brasil quando falta o
     * código do país. `null` se não parece um celular/fixo válido.
     */
    fun forWhatsApp(raw: String, defaultCountry: String = "55"): String? {
        var digits = raw.filter { it.isDigit() }
        if (raw.trim().startsWith("+")) return digits.takeIf { it.length in 10..15 }
        digits = digits.trimStart('0')
        return when {
            digits.length in 10..11 -> defaultCountry + digits
            digits.startsWith(defaultCountry) && digits.length in 12..13 -> digits
            digits.length in 12..15 -> digits
            else -> null
        }
    }
}
