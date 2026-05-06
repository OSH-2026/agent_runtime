package util

object Logging {
    fun info(message: String) {
        println(message)
    }

    fun error(message: String) {
        System.err.println(message)
    }
}
