package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import java.io.File

@Serializable
data class ReadFileInput(val path: String)

@Serializable
data class ReadFileOutput(val content: String)

class ReadFileAction : Action<ReadFileInput, ReadFileOutput> {
    override suspend fun execute(input: ReadFileInput, ctx: ActionContext): ReadFileOutput {
        val content = File(input.path).readText()
        return ReadFileOutput(content)
    }
}
