package br.com.anotai.actions

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** Notas ditadas ("anota aí…"), guardadas só no aparelho. */
class NoteStore(context: Context) {
    data class Note(val text: String, val createdAt: Long)

    private val prefs = context.getSharedPreferences("anotai_notes", Context.MODE_PRIVATE)

    fun list(): List<Note> {
        val raw = prefs.getString(KEY, "[]") ?: "[]"
        val arr = runCatching { JSONArray(raw) }.getOrDefault(JSONArray())
        return (0 until arr.length()).mapNotNull { i ->
            arr.optJSONObject(i)?.let { Note(it.optString("text"), it.optLong("at")) }
        }.sortedByDescending { it.createdAt }
    }

    fun add(text: String) {
        val arr = runCatching { JSONArray(prefs.getString(KEY, "[]")) }.getOrDefault(JSONArray())
        arr.put(JSONObject().put("text", text).put("at", System.currentTimeMillis()))
        prefs.edit().putString(KEY, arr.toString()).apply()
    }

    fun clear() = prefs.edit().remove(KEY).apply()

    private companion object {
        const val KEY = "notes"
    }
}
