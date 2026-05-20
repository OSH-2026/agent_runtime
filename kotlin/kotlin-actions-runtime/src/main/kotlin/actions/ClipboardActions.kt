package actions

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class ClipboardCopyInput(
    val label: String = "action_runtime",
    val text: String,
)

@Serializable
data class ClipboardCopyOutput(val copied: Boolean)

class ClipboardCopyAction : Action<ClipboardCopyInput, ClipboardCopyOutput> {
    override suspend fun execute(input: ClipboardCopyInput, ctx: ActionContext): ClipboardCopyOutput {
        val clipboard = ctx.appContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText(input.label, input.text))
        return ClipboardCopyOutput(copied = true)
    }
}

@Serializable
data class ClipboardReadInput(val unused: Boolean = true)

@Serializable
data class ClipboardReadOutput(
    val text: String,
    val hasText: Boolean,
)

class ClipboardReadAction : Action<ClipboardReadInput, ClipboardReadOutput> {
    override suspend fun execute(input: ClipboardReadInput, ctx: ActionContext): ClipboardReadOutput {
        val clipboard = ctx.appContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val text = if (clipboard.hasPrimaryClip()) {
            clipboard.primaryClip?.getItemAt(0)?.coerceToText(ctx.appContext)?.toString() ?: ""
        } else {
            ""
        }
        return ClipboardReadOutput(text = text, hasText = text.isNotEmpty())
    }
}
