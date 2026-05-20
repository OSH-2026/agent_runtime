package runtime

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import util.Logging

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        val alarmId = intent?.getStringExtra(EXTRA_ALARM_ID) ?: ""
        val message = intent?.getStringExtra(EXTRA_ALARM_MESSAGE) ?: ""
        Logging.info("Alarm fired: id=$alarmId message=$message")
    }

    companion object {
        const val EXTRA_ALARM_ID = "runtime.alarm.id"
        const val EXTRA_ALARM_MESSAGE = "runtime.alarm.message"
    }
}
