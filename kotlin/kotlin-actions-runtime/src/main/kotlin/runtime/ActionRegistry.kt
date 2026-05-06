package runtime

import api.Action
import kotlinx.serialization.KSerializer

class ActionRegistry {
    private val actions = mutableMapOf<String, ActionSpec<*, *>>()

    fun <I : Any, O : Any> register(
        name: String,
        action: Action<I, O>,
        inputSerializer: KSerializer<I>,
        outputSerializer: KSerializer<O>,
    ) {
        actions[name] = ActionSpec(action, inputSerializer, outputSerializer)
    }

    fun get(name: String): ActionSpec<*, *> {
        return actions[name] ?: throw IllegalArgumentException("Action not found: $name")
    }
}

data class ActionSpec<I : Any, O : Any>(
    val action: Action<I, O>,
    val inputSerializer: KSerializer<I>,
    val outputSerializer: KSerializer<O>,
)
