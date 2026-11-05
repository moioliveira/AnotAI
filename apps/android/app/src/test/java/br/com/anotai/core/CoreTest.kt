package br.com.anotai.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.LocalTime

class WakePhraseTest {
    @Test fun `bom dia AnotAI ativa e separa o comando`() {
        val w = WakePhrase.find("Bom dia AnotAI, ative o despertador para as 7")!!
        assertEquals(Greeting.MORNING, w.greeting)
        assertEquals("ative o despertador para as 7", w.command)
    }

    @Test fun `variações que o reconhecedor escreve`() {
        for (heard in listOf("boa tarde anota aí mande", "Boa noite, Anota Aí: mande", "oi anotaí mande", "ok a nota ai mande")) {
            assertEquals(heard, "mande", WakePhrase.find(heard)?.command)
        }
        assertEquals(Greeting.NIGHT, WakePhrase.find("Boa noite, Anota Aí: qual a agenda")!!.greeting)
    }

    @Test fun `ruído antes da frase e só o nome`() {
        assertEquals("que horas são", WakePhrase.find("hum então bom dia AnotAI que horas são")!!.command)
        val onlyName = WakePhrase.find("Bom dia AnotAI")!!
        assertEquals("", onlyName.command)
        assertEquals(Greeting.MORNING, onlyName.greeting)
    }

    @Test fun `sem o nome não ativa`() {
        assertNull(WakePhrase.find("bom dia pessoal, vamos começar"))
    }
}

class TimeParserTest {
    private val today = LocalDate.of(2026, 11, 5) // quinta-feira

    @Test fun `horários falados`() {
        val cases = mapOf(
            "às 7" to LocalTime.of(7, 0),
            "às 7:30" to LocalTime.of(7, 30),
            "6h45" to LocalTime.of(6, 45),
            "sete e meia" to LocalTime.of(7, 30),
            "às seis e quinze da manhã" to LocalTime.of(6, 15),
            "às 3 da tarde" to LocalTime.of(15, 0),
            "às oito da noite" to LocalTime.of(20, 0),
            "ao meio-dia" to LocalTime.of(12, 0),
            "meia-noite" to LocalTime.of(0, 0),
            "para as 22 horas" to LocalTime.of(22, 0),
            "vinte e uma horas" to LocalTime.of(21, 0),
        )
        for ((text, expected) in cases) assertEquals(text, expected, TimeParser.clock(text))
    }

    @Test fun `duração não é horário`() {
        assertNull(TimeParser.clock("daqui a 10 minutos"))
        assertNull(TimeParser.clock("por 5 minutos"))
        assertEquals(600L, TimeParser.durationSeconds("daqui a 10 minutos"))
        assertEquals(1800L, TimeParser.durationSeconds("meia hora"))
        assertEquals(5400L, TimeParser.durationSeconds("uma hora e meia"))
        assertEquals(90L, TimeParser.durationSeconds("1 minuto e 30 segundos"))
    }

    @Test fun `dias`() {
        assertEquals(today.plusDays(1), TimeParser.day("qual a agenda de amanhã", today))
        assertEquals(today.plusDays(2), TimeParser.day("depois de amanhã", today))
        assertEquals(LocalDate.of(2026, 11, 6), TimeParser.day("na sexta-feira", today))
        assertEquals(LocalDate.of(2026, 11, 9), TimeParser.day("segunda", today))
        assertEquals(LocalDate.of(2026, 11, 20), TimeParser.day("dia 20", today))
        assertEquals(LocalDate.of(2026, 12, 2), TimeParser.day("dia 2", today))
        assertNull(TimeParser.day("qual a agenda", today))
    }
}

class CommandParserTest {
    private val today = LocalDate.of(2026, 11, 5)
    private fun parse(s: String) = CommandParser.parse(s, today)

    // Os três exemplos do pedido.
    @Test fun `ative o despertador`() {
        assertEquals(Command.SetAlarm(LocalTime.of(7, 0)), parse("ative o despertador para as 7"))
        assertEquals(Command.SetAlarm(null), parse("ative o despertador"))
        assertEquals(Command.SetAlarm(LocalTime.of(6, 30)), parse("me acorde às seis e meia"))
    }

    @Test fun `mande uma mensagem com o texto que vou falar`() {
        assertEquals(Command.SendMessage(null, null, null), parse("mande uma mensagem com o texto que vou falar"))
    }

    @Test fun `qual a agenda de amanhã`() {
        assertEquals(Command.Agenda(today.plusDays(1)), parse("qual a agenda de amanhã"))
        assertEquals(Command.Agenda(today), parse("o que eu tenho hoje"))
        assertEquals(Command.Agenda(today), parse("qual a minha agenda"))
    }

    @Test fun `mensagem completa preserva acentos e maiúsculas`() {
        assertEquals(
            Command.SendMessage("Maria", "Já estou chegando, até já", Channel.WHATSAPP),
            parse("manda um zap pra Maria dizendo Já estou chegando, até já"),
        )
        assertEquals(
            Command.SendMessage("João", "a reunião começou", Channel.SMS),
            parse("envie um SMS para o João avisando que a reunião começou"),
        )
        assertEquals(Command.SendMessage("Ana Paula", null, null), parse("mande uma mensagem para a Ana Paula"))
    }

    @Test fun `ligar`() {
        assertEquals(Command.Call("Mãe"), parse("ligue para a Mãe"))
        assertEquals(Command.Call(null), parse("faça uma ligação"))
    }

    @Test fun `lembrete com hora vira alarme com nome`() {
        assertEquals(Command.SetAlarm(LocalTime.of(20, 0), "Tomar o remédio"), parse("me lembre de tomar o remédio às 8 da noite"))
        assertEquals(Command.Note("comprar pão"), parse("me lembre de comprar pão"))
    }

    @Test fun `evento`() {
        assertEquals(
            Command.CreateEvent("Reunião com o João", today.plusDays(1), LocalTime.of(10, 0)),
            parse("marque uma reunião com o João amanhã às 10"),
        )
        val dentist = parse("agende dentista na sexta às 3 da tarde") as Command.CreateEvent
        assertEquals("Dentista", dentist.title)
        assertEquals(LocalTime.of(15, 0), dentist.time)
    }

    @Test fun `uma como artigo e como hora`() {
        assertEquals(
            Command.CreateEvent("Reunião de orçamento", today, LocalTime.of(10, 0)),
            parse("crie uma reunião de orçamento hoje às dez"),
        )
        assertEquals(Command.SetAlarm(LocalTime.of(13, 0)), parse("despertador à uma da tarde"))
        assertEquals(Command.SetAlarm(LocalTime.of(1, 30)), parse("alarme para uma e meia"))
    }

    @Test fun `outros comandos`() {
        assertEquals(Command.SetTimer(300), parse("timer de 5 minutos"))
        assertEquals(Command.Flashlight(true), parse("acenda a lanterna"))
        assertEquals(Command.Flashlight(false), parse("desliga a lanterna"))
        assertEquals(Command.OpenApp("WhatsApp"), parse("abra o WhatsApp"))
        assertEquals(Command.TellTime, parse("que horas são"))
        assertEquals(Command.TellDate, parse("que dia é hoje"))
        assertEquals(Command.Note("comprar leite e café"), parse("anota aí comprar leite e café"))
        assertEquals(Command.Help, parse("o que você sabe fazer"))
        assertTrue(parse("qual a capital da França") is Command.Unknown)
    }

    @Test fun `confirmações`() {
        assertEquals(true, Confirm.parse("sim, pode mandar"))
        assertEquals(false, Confirm.parse("não, cancela"))
        assertNull(Confirm.parse("talvez"))
        assertTrue(Confirm.isCancel("deixa pra lá"))
    }
}

class SpeechAndPhoneTest {
    private val today = LocalDate.of(2026, 11, 5)

    @Test fun `agenda falada`() {
        val items = listOf(
            Speech.AgendaItem("Reunião de vendas", LocalTime.of(9, 0), false),
            Speech.AgendaItem("Dentista", LocalTime.of(14, 30), false),
            Speech.AgendaItem("Aniversário da Ana", null, true),
        )
        assertEquals(
            "Amanhã você tem 3 compromissos: às 9 horas, Reunião de vendas; às 14 e meia, Dentista; Aniversário da Ana, o dia todo.",
            Speech.agenda(items, today.plusDays(1), today),
        )
        assertEquals("Você não tem compromissos amanhã.", Speech.agenda(emptyList(), today.plusDays(1), today))
    }

    @Test fun `horas e durações faladas`() {
        assertEquals("7 horas", Speech.time(LocalTime.of(7, 0)))
        assertEquals("meio-dia", Speech.time(LocalTime.NOON))
        assertEquals("14 e 15", Speech.time(LocalTime.of(14, 15)))
        assertEquals("1 hora e 30 minutos", Speech.duration(5400))
    }

    @Test fun `preposição antes do horário`() {
        assertEquals("às 7 horas", Speech.atTime(LocalTime.of(7, 0)))
        assertEquals("ao meio-dia", Speech.atTime(LocalTime.NOON))
        assertEquals("à meia-noite", Speech.atTime(LocalTime.MIDNIGHT))
        assertEquals("à uma hora", Speech.atTime(LocalTime.of(1, 0)))
        assertEquals("à uma e meia", Speech.atTime(LocalTime.of(1, 30)))
        assertEquals("às 13 horas", Speech.atTime(LocalTime.of(13, 0)))
        assertEquals("para o meio-dia", Speech.forTime(LocalTime.NOON))
        assertEquals("para as 6 e meia", Speech.forTime(LocalTime.of(6, 30)))
    }

    @Test fun `telefone para WhatsApp`() {
        assertEquals("5511987654321", Phone.forWhatsApp("(11) 98765-4321"))
        assertEquals("5511987654321", Phone.forWhatsApp("011 98765-4321"))
        assertEquals("5511987654321", Phone.forWhatsApp("+55 11 98765-4321"))
        assertEquals("351912345678", Phone.forWhatsApp("+351 912 345 678"))
        assertNull(Phone.forWhatsApp("190"))
    }
}
