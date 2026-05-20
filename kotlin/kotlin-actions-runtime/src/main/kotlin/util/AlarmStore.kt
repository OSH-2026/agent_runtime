package util

import actions.AlarmEntry
import android.content.Context
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

object AlarmStore {
    private const val PREFS = "action_runtime_alarms"
    private const val KEY_ALARMS = "alarms"

    private val json = Json { ignoreUnknownKeys = true }

    fun add(context: Context, entry: AlarmEntry) {
        val current = load(context).toMutableList()
        current.add(entry)
        save(context, current)
    }

    fun load(context: Context): List<AlarmEntry> {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val raw = prefs.getString(KEY_ALARMS, null) ?: return emptyList()
        return runCatching {
            json.decodeFromString(ListSerializer(AlarmEntry.serializer()), raw)
        }.getOrDefault(emptyList())
    }

    private fun save(context: Context, alarms: List<AlarmEntry>) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val raw = json.encodeToString(ListSerializer(AlarmEntry.serializer()), alarms)
        prefs.edit().putString(KEY_ALARMS, raw).apply()
    }
}
