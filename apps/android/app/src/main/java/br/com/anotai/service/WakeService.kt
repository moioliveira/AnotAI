package br.com.anotai.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import br.com.anotai.AnotAIApp
import br.com.anotai.R
import br.com.anotai.ui.MainActivity

/**
 * Escuta contínua (experimental): serviço em primeiro plano com notificação
 * fixa, mantendo o microfone ativo para "Bom dia AnotAI…". O Android exige
 * que ele seja iniciado com o app aberto.
 */
class WakeService : Service() {
    private val assistant get() = (application as AnotAIApp).assistant

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        val notification = notification()
        // O tipo "microfone" existe a partir do Android 11 (e é obrigatório no 14+).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        assistant.setContinuous(true)
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        assistant.setContinuous(false)
        super.onDestroy()
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Escuta contínua", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Mostra quando o AnotAI está ouvindo a frase de ativação."
            },
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val stop = PendingIntent.getService(
            this, 1, Intent(this, WakeService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_mic)
            .setContentTitle("AnotAI está ouvindo")
            .setContentText("Diga \"Bom dia AnotAI\" e faça seu pedido.")
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null, "Parar", stop).build())
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val CHANNEL = "wake"
        private const val NOTIFICATION_ID = 1
        private const val ACTION_STOP = "br.com.anotai.STOP_WAKE"

        fun start(context: Context) =
            context.startForegroundService(Intent(context, WakeService::class.java))

        fun stop(context: Context) = context.stopService(Intent(context, WakeService::class.java))
    }
}
