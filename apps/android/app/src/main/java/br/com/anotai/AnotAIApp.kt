package br.com.anotai

import android.app.Application
import br.com.anotai.assistant.Assistant

/** Guarda o assistente único do processo (tela, atalhos e escuta contínua). */
class AnotAIApp : Application() {
    lateinit var assistant: Assistant
        private set

    override fun onCreate() {
        super.onCreate()
        assistant = Assistant(this)
    }
}
