package br.com.anotai.speech

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import java.util.Locale
import java.util.concurrent.atomic.AtomicInteger

/** Voz do assistente (TextToSpeech do Android, pt-BR). */
class VoiceOutput(context: Context) {
    private val main = Handler(Looper.getMainLooper())
    private val callbacks = mutableMapOf<String, () -> Unit>()
    private val ids = AtomicInteger()
    private var ready = false
    private val queue = mutableListOf<Pair<String, () -> Unit>>()

    private lateinit var tts: TextToSpeech

    init {
        tts = TextToSpeech(context.applicationContext) { status ->
            main.post {
                if (status == TextToSpeech.SUCCESS) {
                    tts.language = Locale.forLanguageTag("pt-BR")
                    ready = true
                    val pending = queue.toList()
                    queue.clear()
                    pending.forEach { (text, done) -> speak(text, done) }
                } else {
                    val pending = queue.toList()
                    queue.clear()
                    pending.forEach { (_, done) -> done() }
                }
            }
        }
        tts.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {}
            override fun onDone(utteranceId: String?) = finish(utteranceId)
            @Deprecated("API antiga", ReplaceWith("onError(utteranceId, errorCode)"))
            override fun onError(utteranceId: String?) = finish(utteranceId)
            override fun onError(utteranceId: String?, errorCode: Int) = finish(utteranceId)
        })
    }

    private fun finish(id: String?) {
        main.post { id?.let { callbacks.remove(it)?.invoke() } }
    }

    /** Fala e chama `onDone` ao terminar (também em caso de erro). */
    fun speak(text: String, onDone: () -> Unit = {}) {
        if (!ready) {
            queue += text to onDone
            return
        }
        val id = "u${ids.incrementAndGet()}"
        callbacks[id] = onDone
        if (tts.speak(text, TextToSpeech.QUEUE_FLUSH, null, id) != TextToSpeech.SUCCESS) finish(id)
    }

    fun stop() {
        tts.stop()
        val pending = callbacks.values.toList()
        callbacks.clear()
        pending.forEach { it() }
    }

    fun shutdown() = tts.shutdown()
}
