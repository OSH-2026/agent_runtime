package api

interface Action<I : Any, O : Any> {
    suspend fun execute(input: I, ctx: ActionContext): O
}
