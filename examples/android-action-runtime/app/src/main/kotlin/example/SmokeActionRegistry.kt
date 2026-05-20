package example

import actions.registerBuiltinActions
import actions.registerIntentActions
import runtime.ActionRegistry

/**
 * Mirrors [runtime.ActionRuntime.registerDefaults] registration list.
 * Uses [actions.registerBuiltinActions] and [actions.registerIntentActions].
 */
fun buildSmokeActionRegistry(): ActionRegistry {
    return ActionRegistry().apply {
        registerBuiltinActions()
        registerIntentActions()
    }
}
