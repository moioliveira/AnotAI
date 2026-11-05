package br.com.anotai.actions

import android.Manifest
import android.content.ActivityNotFoundException
import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.net.Uri
import android.provider.AlarmClock
import android.provider.CalendarContract
import android.provider.ContactsContract
import android.telephony.SmsManager
import br.com.anotai.core.Speech
import br.com.anotai.core.fold
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.time.ZoneOffset

/** Ações no aparelho. Cada função devolve se deu certo. */
class DeviceActions(private val context: Context) {
    data class Contact(val name: String, val phone: String)

    fun has(permission: String) =
        context.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private fun start(intent: Intent): Boolean = try {
        context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (_: ActivityNotFoundException) {
        false
    } catch (_: SecurityException) {
        false
    }

    fun setAlarm(time: LocalTime, label: String?): Boolean = start(
        Intent(AlarmClock.ACTION_SET_ALARM)
            .putExtra(AlarmClock.EXTRA_HOUR, time.hour)
            .putExtra(AlarmClock.EXTRA_MINUTES, time.minute)
            .putExtra(AlarmClock.EXTRA_MESSAGE, label ?: "AnotAI")
            .putExtra(AlarmClock.EXTRA_SKIP_UI, true),
    )

    fun setTimer(seconds: Long): Boolean = start(
        Intent(AlarmClock.ACTION_SET_TIMER)
            .putExtra(AlarmClock.EXTRA_LENGTH, seconds.toInt())
            .putExtra(AlarmClock.EXTRA_MESSAGE, "AnotAI")
            .putExtra(AlarmClock.EXTRA_SKIP_UI, true),
    )

    /**
     * Procura o contato pelo nome falado: nome exato, depois começo do nome,
     * depois qualquer parte. Prefere celular. `null` se não achou (ou sem
     * permissão).
     */
    fun findContact(spoken: String): Contact? {
        if (!has(Manifest.permission.READ_CONTACTS)) return null
        val query = fold(spoken).trim()
        val found = mutableListOf<Triple<Contact, Int, Boolean>>()
        val projection = arrayOf(
            ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME,
            ContactsContract.CommonDataKinds.Phone.NUMBER,
            ContactsContract.CommonDataKinds.Phone.TYPE,
        )
        context.contentResolver.query(
            ContactsContract.CommonDataKinds.Phone.CONTENT_URI, projection, null, null, null,
        )?.use { c ->
            while (c.moveToNext()) {
                val name = c.getString(0) ?: continue
                val number = c.getString(1) ?: continue
                val folded = fold(name)
                val rank = when {
                    folded == query -> 0
                    folded.startsWith("$query ") || folded.startsWith(query) -> 1
                    Regex("\\b${Regex.escape(query)}\\b").containsMatchIn(folded) -> 2
                    else -> continue
                }
                val mobile = c.getInt(2) == ContactsContract.CommonDataKinds.Phone.TYPE_MOBILE
                found += Triple(Contact(name, number), rank, mobile)
            }
        }
        return found.sortedWith(compareBy({ it.second }, { !it.third })).firstOrNull()?.first
    }

    fun hasWhatsApp(): Boolean = WHATSAPP_PACKAGES.any { installed(it) }

    private fun installed(pkg: String) = try {
        context.packageManager.getPackageInfo(pkg, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }

    /** Abre a conversa no WhatsApp com o texto pronto (o envio é um toque). */
    fun openWhatsApp(phoneInternational: String, text: String): Boolean {
        val uri = Uri.parse("https://api.whatsapp.com/send?phone=$phoneInternational&text=${Uri.encode(text)}")
        val pkg = WHATSAPP_PACKAGES.firstOrNull { installed(it) }
        return start(Intent(Intent.ACTION_VIEW, uri).apply { pkg?.let { setPackage(it) } })
    }

    fun sendSms(phone: String, text: String): Boolean {
        if (!has(Manifest.permission.SEND_SMS)) return false
        return try {
            @Suppress("DEPRECATION")
            val sms = context.getSystemService(SmsManager::class.java) ?: SmsManager.getDefault()
            sms.sendMultipartTextMessage(phone, null, sms.divideMessage(text), null, null)
            true
        } catch (_: Exception) {
            false
        }
    }

    /** Liga direto com permissão; sem ela, abre o discador com o número. */
    fun call(phone: String): Boolean {
        val uri = Uri.fromParts("tel", phone, null)
        return if (has(Manifest.permission.CALL_PHONE)) {
            start(Intent(Intent.ACTION_CALL, uri))
        } else {
            start(Intent(Intent.ACTION_DIAL, uri))
        }
    }

    /** Compromissos do dia, em ordem. `null` sem permissão. */
    fun agenda(date: LocalDate): List<Speech.AgendaItem>? {
        if (!has(Manifest.permission.READ_CALENDAR)) return null
        val zone = ZoneId.systemDefault()
        val begin = date.atStartOfDay(zone).toInstant().toEpochMilli()
        val end = date.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
        val builder = CalendarContract.Instances.CONTENT_URI.buildUpon()
        ContentUris.appendId(builder, begin - DAY_MS)
        ContentUris.appendId(builder, end + DAY_MS)
        val items = mutableListOf<Pair<Long, Speech.AgendaItem>>()
        context.contentResolver.query(
            builder.build(),
            arrayOf(
                CalendarContract.Instances.TITLE,
                CalendarContract.Instances.BEGIN,
                CalendarContract.Instances.ALL_DAY,
                CalendarContract.Instances.VISIBLE,
            ),
            null, null, "${CalendarContract.Instances.BEGIN} ASC",
        )?.use { c ->
            while (c.moveToNext()) {
                if (c.getInt(3) == 0) continue
                val start = c.getLong(1)
                val allDay = c.getInt(2) == 1
                // Eventos de dia inteiro ficam gravados em UTC.
                val day = if (allDay) {
                    Instant.ofEpochMilli(start).atZone(ZoneOffset.UTC).toLocalDate()
                } else {
                    Instant.ofEpochMilli(start).atZone(zone).toLocalDate()
                }
                if (day != date) continue
                val time = if (allDay) null else Instant.ofEpochMilli(start).atZone(zone).toLocalTime()
                val title = c.getString(0)?.ifBlank { null } ?: "sem título"
                items += start to Speech.AgendaItem(title, time, allDay)
            }
        }
        return items.sortedWith(compareBy({ !it.second.allDay }, { it.first })).map { it.second }
    }

    /** Abre o app de agenda com o evento preenchido (o usuário confere e salva). */
    fun createEvent(title: String, date: LocalDate, time: LocalTime?): Boolean {
        val zone = ZoneId.systemDefault()
        val intent = Intent(Intent.ACTION_INSERT, CalendarContract.Events.CONTENT_URI)
            .putExtra(CalendarContract.Events.TITLE, title)
        if (time == null) {
            val start = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
            intent.putExtra(CalendarContract.EXTRA_EVENT_ALL_DAY, true)
                .putExtra(CalendarContract.EXTRA_EVENT_BEGIN_TIME, start)
                .putExtra(CalendarContract.EXTRA_EVENT_END_TIME, start + DAY_MS)
        } else {
            val start = date.atTime(time).atZone(zone).toInstant().toEpochMilli()
            intent.putExtra(CalendarContract.EXTRA_EVENT_BEGIN_TIME, start)
                .putExtra(CalendarContract.EXTRA_EVENT_END_TIME, start + HOUR_MS)
        }
        return start(intent)
    }

    fun torch(on: Boolean): Boolean = try {
        val cm = context.getSystemService(CameraManager::class.java)
        val id = cm.cameraIdList.firstOrNull {
            cm.getCameraCharacteristics(it).get(CameraCharacteristics.FLASH_INFO_AVAILABLE) == true
        }
        if (id == null) false else {
            cm.setTorchMode(id, on)
            true
        }
    } catch (_: Exception) {
        false
    }

    /** Abre um app pelo nome falado. Devolve o nome encontrado. */
    fun openApp(spoken: String): String? {
        val pm = context.packageManager
        val query = fold(spoken).trim()
        val launchers = pm.queryIntentActivities(
            Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER), 0,
        )
        val best = launchers.mapNotNull { info ->
            val label = info.loadLabel(pm).toString()
            val folded = fold(label)
            val rank = when {
                folded == query -> 0
                folded.startsWith(query) -> 1
                folded.contains(query) || query.contains(folded) -> 2
                else -> return@mapNotNull null
            }
            Triple(label, info.activityInfo.packageName, rank)
        }.minByOrNull { it.third } ?: return null
        val intent = pm.getLaunchIntentForPackage(best.second) ?: return null
        return if (start(intent)) best.first else null
    }

    companion object {
        private val WHATSAPP_PACKAGES = listOf("com.whatsapp", "com.whatsapp.w4b")
        private const val HOUR_MS = 3_600_000L
        private const val DAY_MS = 86_400_000L
    }
}
