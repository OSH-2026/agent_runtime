package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import java.net.HttpURLConnection
import java.net.URL

@Serializable
data class HttpRequest(val url: String)

@Serializable
data class HttpResponse(val body: String, val status: Int)

class HttpCallAction : Action<HttpRequest, HttpResponse> {
    override suspend fun execute(input: HttpRequest, ctx: ActionContext): HttpResponse {
        val connection = URL(input.url).openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "GET"
            connection.connectTimeout = 10_000
            connection.readTimeout = 10_000
            val status = connection.responseCode
            val body = connection.inputStream.bufferedReader().use { it.readText() }
            return HttpResponse(body, status)
        } finally {
            connection.disconnect()
        }
    }
}
