package br.com.anotai.core

import java.time.LocalDate

/**
 * Interpreta o comando falado (já sem a frase de ativação) por regras. Os
 * padrões rodam no texto "dobrado" (minúsculo, sem acentos), e os trechos
 * livres (nome do contato, texto da mensagem, nota) são recortados do texto
 * original pelos mesmos índices, preservando acentos e maiúsculas.
 */
object CommandParser {
    private fun has(t: String, pattern: String) = Regex(pattern).containsMatchIn(t)

    fun parse(raw: String, today: LocalDate): Command {
        val text = raw.trim().trimEnd('.', '!', '?')
        val t = fold(text)
        return when {
            has(t, "\\blanterna\\b") ->
                Command.Flashlight(on = !has(t, "\\b(desliga\\w*|apaga\\w*)\\b"))
            has(t, "^(?:por favor\\s+)?(?:abr[aei]r?|inicie|iniciar|execute)\\s+") -> openApp(text, t)
            has(t, "\\b(?:que horas|hora certa|horas agora)\\b") -> Command.TellTime
            has(t, "\\b(?:que dia e hoje|que data|qual (?:a )?data|dia de hoje)\\b") -> Command.TellDate
            has(t, "\\b(?:o que voce (?:faz|sabe)|ajuda|comandos|o que (?:posso|eu posso) (?:pedir|falar))\\b") -> Command.Help
            has(t, "\\b(?:timer|temporizador|cronometro)\\b") -> Command.SetTimer(TimeParser.durationSeconds(text))
            has(t, "\\b(?:desperta\\w*|alarme|me acord\\w*)\\b") -> alarm(text, today)
            has(t, "\\b(?:lembre-?\\s?me|me lembr\\w*|lembrete)\\b") -> reminder(text, t, today)
            has(t, "\\b(?:mensagem|mande|manda|envie|envia|mandar|enviar|whatsapp|zap|sms|torpedo|escrev[ae])\\b") ->
                message(text, t)
            has(t, "\\b(?:ligu?e|liga|ligar|ligacao|telefone|telefona)\\b") -> Command.Call(
                after(text, t, "\\b(?:ligu?e|liga|ligar|telefone|telefona)\\s+(?:para|pra|pro)\\s*(?:o\\s+|a\\s+)?"),
            )
            has(t, "\\b(?:agenda|compromissos?|reunioes|eventos|o que (?:eu )?tenho)\\b") &&
                !has(t, "\\b(?:marque|marca|agende|agenda (?:uma|um)|crie|cria|adicione|coloque)\\b") ->
                Command.Agenda(TimeParser.day(text, today) ?: today)
            has(t, "\\b(?:marque|marca|agende|agendar|crie|cria|adicione|coloque)\\b") -> event(text, t, today)
            has(t, "^(?:anot[ae]|anota ai|anotar|nota|tome nota|escreva)\\b") -> Command.Note(
                after(text, t, "^(?:anot[ae]r?(?:\\s+ai)?|tome nota|nota|escreva)\\s*(?:que\\s+|de\\s+|:\\s*)?"),
            )
            else -> Command.Unknown(text)
        }
    }

    /** Texto original depois do trecho que casa `pattern` (no dobrado). */
    private fun after(text: String, t: String, pattern: String): String? {
        val m = Regex(pattern).find(t) ?: return null
        return text.substring(m.range.last + 1).trim().trim(',', ':', '.').ifBlank { null }
    }

    private fun openApp(text: String, t: String): Command {
        val name = after(text, t, "^(?:por favor\\s+)?(?:abr[aei]r?|inicie|iniciar|execute)\\s+(?:o\\s+app\\s+|o\\s+aplicativo\\s+|o\\s+|a\\s+)?")
        return if (name == null) Command.Unknown(text) else Command.OpenApp(name)
    }

    private fun alarm(text: String, today: LocalDate): Command {
        val relative = TimeParser.durationSeconds(text)
        val time = TimeParser.clock(text)
            ?: relative?.let { java.time.LocalTime.now().plusSeconds(it).withSecond(0).withNano(0) }
        return Command.SetAlarm(time)
    }

    /** "me lembre de tomar remédio às 8": com hora vira alarme com nome; sem hora, nota. */
    private fun reminder(text: String, t: String, today: LocalDate): Command {
        val what = after(text, t, "\\b(?:lembre-?\\s?me|me lembr\\w*|lembrete)\\s*(?:de\\s+|para\\s+|que\\s+)?")
            ?.let { stripTimeExpressions(it) }
        val time = TimeParser.clock(text)
        return if (time != null) Command.SetAlarm(time, what?.capitalizeFirst()) else Command.Note(what)
    }

    private val TEXT_MARKERS =
        "dizendo|falando|com o texto|com a mensagem|escrito|que diz|avisando que|avisando"
    private val CHANNEL_WORDS =
        "(?:no|pelo|por|via)\\s+(?:whatsapp|whats|zap|sms|mensagem de texto|torpedo)"

    private fun message(text: String, t: String): Command {
        val channel = when {
            has(t, "\\b(?:whatsapp|whats|zap)\\b") -> Channel.WHATSAPP
            has(t, "\\b(?:sms|torpedo|mensagem de texto)\\b") -> Channel.SMS
            else -> null
        }
        // "com o texto que vou falar": o texto vem depois, perguntado.
        val dictateLater = has(t, "\\bque (?:eu )?vou (?:falar|dizer|ditar)\\b")
        val body = if (dictateLater) null else {
            Regex("\\b(?:$TEXT_MARKERS)\\b\\s*:?\\s*").find(t)?.let { m ->
                text.substring(m.range.last + 1).trim().ifBlank { null }
            }
        }
        val contact = Regex(
            "\\b(?:para|pra|pro)\\s+(?:o\\s+|a\\s+)?(.+?)(?=\\s+(?:$TEXT_MARKERS)\\b|\\s+$CHANNEL_WORDS\\b|\\s+com\\s+o\\s+texto\\b|[,:]|$)",
        ).find(t)?.let { m ->
            text.substring(m.groups[1]!!.range).trim().ifBlank { null }
        }
        return Command.SendMessage(contact, body, channel)
    }

    private val TIME_EXPRESSIONS = listOf(
        "\\b(?:as|a|para as|pras|pra)\\s+\\d{1,2}(?:\\s*(?::|h)\\s*\\d{2})?(?:\\s*(?:h|horas?))?(?:\\s+e\\s+(?:meia|\\d{1,2}))?(?:\\s+da\\s+(?:manha|tarde|noite))?",
        "\\b\\d{1,2}\\s*(?::|h)\\s*\\d{2}\\b",
        "\\b\\d{1,2}\\s*(?:h|horas?)\\b(?:\\s+e\\s+(?:meia|\\d{1,2}))?",
        "\\b(?:ao\\s+|no\\s+)?meio[ -]?dia\\b",
        "\\b(?:a\\s+)?meia[ -]?noite\\b",
        "\\bdepois\\s+de\\s+amanha\\b", "\\bamanha\\b", "\\bhoje\\b",
        "\\b(?:na|no|nesta|neste|proxima|proximo)?\\s*(?:segunda|terca|quarta|quinta|sexta|sabado|domingo)(?:-feira| feira)?\\b",
        "\\bdia\\s+\\d{1,2}\\b",
    )

    /**
     * Remove horários e dias de um trecho do texto original, para sobrar só
     * o assunto ("reunião com o João amanhã às 10" → "reunião com o João").
     * Os padrões rodam no texto dobrado com dígitos, que tem o mesmo tamanho
     * do original; o corte é feito nos dois pelos mesmos índices.
     */
    private fun stripTimeExpressions(original: String): String {
        var s = original
        var f = wordsToDigits(fold(s))
        for (p in TIME_EXPRESSIONS) {
            while (true) {
                val m = Regex(p).find(f) ?: break
                // Inclui o preenchimento de wordsToDigits ("dez" → "10 "):
                // espaço no texto convertido, letra no original.
                var end = m.range.last + 1
                while (end < f.length && f[end] == ' ' && !s[end].isWhitespace()) end++
                s = s.removeRange(m.range.first, end)
                f = f.removeRange(m.range.first, end)
            }
        }
        return s.replace(Regex("\\s{2,}"), " ").trim().trim(',', '.', ':')
    }

    private fun event(text: String, t: String, today: LocalDate): Command {
        val rest = after(text, t, "\\b(?:marque|marca|agende|agendar|crie|cria|adicione|coloque)\\s+(?:na\\s+agenda\\s+|no\\s+calendario\\s+)?")
        val title = rest?.let { stripTimeExpressions(it) }
            ?.replace(Regex("^(?:um|uma)\\s+(?:evento|compromisso)\\s*(?:de|chamado|:)?\\s*", RegexOption.IGNORE_CASE), "")
            ?.replace(Regex("^(?:um|uma|o|a)\\s+", RegexOption.IGNORE_CASE), "")
            ?.replace(Regex("\\s+(?:na|no)\\s+(?:agenda|calendario|calendário)$", RegexOption.IGNORE_CASE), "")
            ?.trim()
            ?.ifBlank { null }
            ?.capitalizeFirst()
        return Command.CreateEvent(title, TimeParser.day(text, today), TimeParser.clock(text))
    }
}
